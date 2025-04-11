use std::path::Path;

use lazy_regex::{Lazy, Regex, lazy_regex};
use wildmatch::WildMatch;

static VAR_REGEX: Lazy<Regex> = lazy_regex!(r"\$\{?([a-zA-Z_][a-zA-Z0-9_]*)\}?");

/// Tl;dr trycmd is basically useless if you have any kind of dynamic data in your tests.
/// Just read this <https://github.com/assert-rs/snapbox/issues/365> and you'll see why.
/// They talk about "having a shel" (???) Bruh, just replace `${VAR_NAME}`` with env var name
///
/// So this is a workaround for that. We just replace the env vars in the test files with the actual values
/// and we write them to a `{name}.eval.trycmd` file.
///
/// ### Arguments
///
/// - `vars` - The variables to replace in the trycmd files
/// - `vars_by_file` - The variables to replace in the trycmd files, by file (file_name -> vars)
/// - `p` - The path to the directory containing the trycmd files
/// - `output_dir` - The directory to write the evaluated trycmd files
/// - `glob` - The glob pattern to match the trycmd files
pub fn eval_trycmd<'a, V>(vars: V, p: &Path, output_dir: &Path, glob: &str) -> anyhow::Result<()>
where
    V: std::iter::IntoIterator<Item = (&'a str, String)>,
{
    let glob = WildMatch::new(glob);
    let vars = vars.into_iter().collect::<Vec<_>>();
    // find files
    for entry in std::fs::read_dir(p)? {
        let entry = entry?;
        let path = entry.path();
        let Some(filename) = path.file_name().and_then(|f| f.to_str()) else {
            continue;
        };

        if filename.ends_with(".eval.trycmd") {
            continue;
        }

        if glob.matches(filename) {
            // read file
            let content = std::fs::read_to_string(&path)?;
            // replace vars
            let content = replace_vars(&content, &vars);

            // get out file path
            let basename = path
                .file_stem()
                .expect("Could not get file stem from trycmd file")
                .to_str()
                .unwrap();
            let out_path = output_dir.join(format!("{}.eval.trycmd", basename));

            // write file
            println!("{content}");
            std::fs::write(out_path, content)?;
        }
    }

    Ok(())
}

/// Replace the variables in the content with the values from the vars
fn replace_vars(content: &str, vars: &[(&str, String)]) -> String {
    VAR_REGEX
        .replace_all(content, |caps: &lazy_regex::Captures| {
            let var_name = caps.get(1).unwrap().as_str();
            match vars
                .iter()
                .find(|(name, _)| name == &var_name)
                .map(|(_, value)| value.to_string())
            {
                Some(value) => value,
                None => panic!("Variable `{var_name}` not found"),
            }
        })
        .to_string()
}
