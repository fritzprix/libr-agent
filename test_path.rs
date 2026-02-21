use std::path::Path;

fn main() {
    let p = Path::new("/a/./b");
    println!("{:?}", p.components().collect::<Vec<_>>());
}
