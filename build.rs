use std::{env, io};

use winresource::WindowsResource;

fn main() -> io::Result<()> {
    if env::var_os("CARGO_CFG_WINDOWS").is_some() {
        WindowsResource::new().set_icon("resources/icon.ico").compile()?;
    }
    Ok(())
}
