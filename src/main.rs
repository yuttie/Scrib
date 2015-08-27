extern crate docopt;
extern crate rustc_serialize;

use docopt::Docopt;

const USAGE: &'static str = "
Fine-grained.

Usage:
    fine add <text>...
    fine (-h | --help)
    fine --version

Options:
    -h, --help  Show this message.
    --version   Show version.
";

#[derive(Debug, RustcDecodable)]
struct Args {
    cmd_add: bool,
    arg_text: Vec<String>,
}

fn main() {
    let args: Args = Docopt::new(USAGE)
                            .and_then(|d| d.decode())
                            .unwrap_or_else(|e| e.exit());
    if args.cmd_add {
        println!("{}", args.arg_text.join(" "));
    }
}
