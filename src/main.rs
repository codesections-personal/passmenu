use clap::{crate_name, crate_version, App, Arg, ArgMatches};
use copypasta::{
    x11_clipboard::{Primary, X11ClipboardContext},
    ClipboardContext, ClipboardProvider,
};
use d5_cli::D5;
use ssh_home::SshHome;
use std::{error::Error, net::Ipv4Addr, ops::Not};
use utils::{dependencies, sh, Die};

fn main() {
    #[rustfmt::skip]
    let cli = App::new(crate_name!())
        .version(crate_version!())
        .about("Interactively select a username (via dmenu) and copy the corresponding password \
             to both primary and selection.  The copied password will be cleared in 45 seconds.")
        .arg("[USER] 'A username to use (instead of prompting via dmenu)'")
        .arg("-s --silent 'Suppresses printing the password to stdout'")
        .arg("-x --no-clip 'Suppresses copying the password to the clipboard'")
        .arg("--ip [IP_ADDRESS] 'The IP address to use (instead of getting it via d5)'")
        .arg(Arg::from("-l --line-range [LINES] 'Line(s) to print or copy'").default_value("1"))
        .arg(Arg::from("--src 'Prints this program's source to stdout'"))
        .after_help("Note: a line range can be either a single line number or a range of line \
             numbers (e.g., `2-5`).  Line numbers are 1-indexed and ranges are inclusive.")
        .get_matches();
    run(cli).unwrap_or_die();
}

fn run(cli: ArgMatches) -> Result<(), Box<dyn Error>> {
    if cli.is_present("src") {
        print!("/// main.rs\n{}", include_str!("main.rs"));
        return Ok(());
    }
    dependencies(vec!["echo", "dmenu"])?;
    let (start, end) = parse_range(cli.value_of("line-range").expect("default"))?;
    let ip = match cli.value_of("ip") {
        Some(ip) => ip
            .parse()
            .map_err(|_| format!("{} is not a valid IP address", ip))?,
        None => {
            let mut d5 = D5::new();
            d5.password = cli.value_of("pass");
            d5.try_ip()?
        }
    };
    let cmd = format!(
        r#"pass "{username}""#,
        username = match cli.value_of("USER") {
            Some(user) => user.to_string(),
            None => get_username_from_remote_pw_store(ip)?,
        }
    );
    let mut ssh_home = SshHome::new(ip);
    ssh_home.command = Some(&cmd);

    let (pass_entry, _err) = ssh_home.run()?;
    let password = pass_entry
        .lines()
        .skip(start)
        .take(end - start)
        .collect::<Vec<&str>>()
        .join("\n");

    if cli.is_present("silent").not() {
        println!("{}", password);
    }
    if cli.is_present("no-clip").not() {
        X11ClipboardContext::<Primary>::new()?.set_contents(password.clone())?;
        ClipboardContext::new()?.set_contents(password)?;
        std::thread::sleep(std::time::Duration::from_secs(45));
    }
    Ok(())
}

fn parse_range(input: &str) -> Result<(usize, usize), Box<dyn Error>> {
    let parse_line = |s: &str| -> Result<usize, Box<dyn Error>> {
        Ok(s.parse::<usize>().map_err(|_| {
            r"Invalid line range.
Please specify either a single line or beginning and ending lines separated a hyphen."
        })?)
    };

    let line = input.split("-").collect::<Vec<&str>>();
    match line.len() {
        1 => {
            let line = parse_line(line[0])?;
            Ok((line - 1, line))
        }
        2 => {
            let start = parse_line(line[0])?;
            let end = parse_line(line[1])?;
            Ok((std::cmp::min(start, end) - 1, std::cmp::max(start, end)))
        }
        _ => Err(r"Invalid line range – too many hyphens in line range.
Please specify either a single line or beginning and ending lines separated a hyphen."
            .into()),
    }
}

fn get_username_from_remote_pw_store(ip: Ipv4Addr) -> Result<String, Box<dyn Error>> {
    let mut ssh_home = SshHome::new(ip);
    ssh_home.command = Some(r#"find ~/.password-store/ -printf "%P\n""#);
    let (filenames, _err) = ssh_home.run()?;

    let (username_selected_via_dmenu, _err) = sh(&format!(
        r#"echo "{usernames}" | dmenu"#,
        usernames = filenames
            .lines()
            .filter(|name_or_dir| name_or_dir.ends_with(".gpg")) // drop directories & git
            .collect::<Vec<&str>>()
            .join("\n")
            .replace(".gpg", "")
            .replace("./", "")
    ))?;
    Ok(username_selected_via_dmenu.trim_end().to_string())
}
