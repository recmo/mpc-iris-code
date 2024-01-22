#![feature(array_chunks)]

mod main;

use criterion::Criterion;

fn main() {
    let mut criterion = Criterion::default().configure_from_args();
    main::benches::group(&mut criterion);
}
