use std::sync::OnceLock;

use reedline::ExternalPrinter;

static PRINTER: OnceLock<ExternalPrinter<String>> = OnceLock::new();

pub(crate) fn install(printer: ExternalPrinter<String>) {
    let _ = PRINTER.set(printer);
}

pub(crate) fn print_line(msg: String) {
    if let Some(p) = PRINTER.get() {
        let _ = p.print(msg);
    } else {
        println!("{msg}");
    }
}

macro_rules! cprintln {
    () => { $crate::printer::print_line(String::new()) };
    ($fmt:literal $(, $arg:expr)* $(,)?) => {
        $crate::printer::print_line(format!($fmt $(, $arg)*))
    };
}
pub(crate) use cprintln;
