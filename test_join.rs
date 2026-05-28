use std::path::PathBuf;

fn main() {
    let base = PathBuf::from("/base/dir/packages");
    let input = "../../../etc/passwd";
    let p = base.join(input);
    println!("Relative: {:?}", p);

    let input2 = "/etc/passwd";
    let p2 = base.join(input2);
    println!("Absolute: {:?}", p2);
}
