use failure::bail;
use mimi::Formatter;
use std::collections::HashMap;
use structopt;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "demo")]
struct Opt {
    /// The mimi format string.
    #[structopt(short = "f", long = "format")]
    format: String,

    /// Arguments to the format string, in the format `key=value`.
    #[structopt(name = "ARG")]
    args: Vec<String>,
}

fn main() -> Result<(), failure::Error> {
    let opt = Opt::from_args();
    match opt.format.parse::<Formatter>() {
        Ok(formatter) => {
            let args: HashMap<String, String> = opt
                .args
                .iter()
                .map(|arg| {
                    let v: Vec<&str> = arg.splitn(2, '=').collect();
                    if v.len() != 2 {
                        bail!("missing = in argument {}", arg);
                    }
                    Ok((v[0].to_owned(), v[1].to_owned()))
                })
                .collect::<Result<_, _>>()?;
            println!("{}", formatter.ansi(&args));
            Ok(())
        }
        Err(err) => {
            println!("{}", err);
            Err(err.into())
        }
    }
}
