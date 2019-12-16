use run_script::run_script;

fn main() {
    let (_code, names, _err) = run_script!(
        r#"
cd $PASSWORD_STORE_DIR
find -name '*' -print | sed 's/\.gpg//g' | sed 's_\./__g' "#
    )
    .unwrap();

    let names: String = names
        .lines()
        .zip(names.lines().skip(1).cycle())
        .filter(|(cur, next)| !next.starts_with(cur))
        .map(|(cur, _)| format!("{}\n", cur))
        .collect();

    let (_, mut name, _) = run_script!(format!(r#"echo "{}" | dmenu"#, names)).unwrap();
    name.pop(); // trim trailing newline

    let (_, entry, _) = run_script!(format!(r#"pass "{}""#, name)).unwrap();
    println!("{}", entry);

    run_script!(format!(
        r#"
echo -n "{pw}" | xclip -selection primary &
echo -n "{pw}" | xclip -selection clipboard &
echo ""
"#,
        pw = entry.lines().nth(0).unwrap().to_string()
    ))
    .unwrap();
}
