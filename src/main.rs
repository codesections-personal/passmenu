use clap::{crate_name, crate_version, App};
use itertools::Itertools;
use run_script::run_script;

fn main() {
    let cli = App::new(crate_name!())
        .version(crate_version!())
        .about("Interactively select a username (via dmenu) and copy the corresponding password (via xclip).")
        .arg("[USER] 'A username to use (instead of prompting via dmenu)'")
        .arg("-s --silent 'Suppresses printing the password to stdout'")
        .arg("-x --no-clip 'Suppresses copying the password to the clipboard'")
        .arg("-l --line-range [LINES] 'Line(s) to print or copy'")
        .after_help("Note: a line range can be either a single line number or a range of line numbers (e.g., `2-5`).  In both cases, line numbers are 1-indexed; if a range is supplied, it is inclusive.")
        .get_matches();

    let (_, ip, _) = run_script!("d5").unwrap();
    let name = cli
        .value_of("USER")
        .map(String::from)
        .unwrap_or_else(|| get_username_from_remote_pw_store(&ip));

    let (code, mut entry, err) =
        run_script!(format!(r#"ssh-home --ip {} -c 'pass "{}"'"#, ip, name)).expect("pass_cmd");
    if code != 0 {
        eprintln!("{}", err);
        std::process::exit(1);
    }

    if let Some(value) = cli.value_of("line-range") {
        let (start, end) = get_range(value);
        entry = entry.lines().skip(start).take(end - start).join("\n");
    }

    if !cli.is_present("silent") {
        println!("{}", entry);
    }
    if !cli.is_present("no-clip") {
        run_script!(format!(
            r#"
echo -n "{pw}" | xclip -selection primary &
echo -n "{pw}" | xclip -selection clipboard &
echo ""
"#,
            pw = entry.lines().nth(0).unwrap_or("").to_string()
        ))
        .expect("xclip_cmd");
    }
}

fn get_range(input: &str) -> (usize, usize) {
    let parse_range = |s: &str| {
        s.parse::<usize>().unwrap_or_else(|_| {
            eprintln!(
                r"Invalid line range.
Please specify either a single line or beginning and ending lines separated a hyphen."
            );
            std::process::exit(1);
        })
    };

    let mut line = input.split("-");
    match line.clone().count() {
        1 => {
            let line = parse_range(line.nth(0).unwrap());
            (line - 1, line)
        }
        2 => {
            let start = parse_range(line.nth(0).unwrap());
            let end = parse_range(line.nth(0).unwrap());
            (std::cmp::min(start, end) - 1, std::cmp::max(start, end))
        }
        _ => (parse_range("Err"), parse_range("Err")),
    }
}

fn get_username_from_remote_pw_store(ip: &str) -> String {
    let cmd = format!(
        r#"ssh-home --ip {} -c 'PASSWORD_STORE_DIR=${{PASSWORD_STORE_DIR:=~/.password-store}}; cd $PASSWORD_STORE_DIR; find -name "*" -print | sed "s/\.gpg//g" | sed "s_\./__g"' "#,
        ip
    );

    let (code, names, err) = run_script!(cmd).expect("find_cmd");
    if code != 0 {
        eprintln!("{}", err);
        std::process::exit(1);
    }

    let names: String = names
        .lines()
        .zip(names.lines().skip(1).cycle())
        .filter(|(cur, next)| !next.starts_with(cur))
        .map(|(cur, _)| cur)
        .join("\n");

    let (_, mut name, _) = run_script!(format!(r#"echo "{}" | dmenu"#, names)).expect("dmenu_cmd");
    name.pop(); // trim trailing newline that dmenu produces
    name
}
