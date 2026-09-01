use std::env;
use std::fs;

use wit_component::ComponentEncoder;

fn main() {
    let mut arguments = env::args_os().skip(1);
    let input = arguments.next().expect("input core module");
    let output = arguments.next().expect("output component");
    assert!(arguments.next().is_none(), "unexpected argument");
    let module = fs::read(input).expect("read core module");
    let component = ComponentEncoder::default()
        .module(&module)
        .expect("decode component metadata")
        .validate(true)
        .encode()
        .expect("encode component");
    fs::write(output, component).expect("write component");
}
