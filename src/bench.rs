use criterion::Criterion;
use mpc_iris_code::benches;

fn main() {
    let mut criterion = Criterion::default().configure_from_args();
    benches::group(&mut criterion);
}
