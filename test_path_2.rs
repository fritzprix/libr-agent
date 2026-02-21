use std::path::Path;

fn main() {
    println!(".: {:?}", Path::new("./foo").components().collect::<Vec<_>>());
    println!("..: {:?}", Path::new("/a/../b").components().collect::<Vec<_>>());
}
