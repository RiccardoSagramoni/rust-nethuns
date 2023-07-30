pub mod bindings;
// pub use bindings::*; // TODO remove after implementing safe wrapper

pub mod macros;
pub mod nmport;

fn main() {
    let _ = bindings::nmport_d::default();
}
