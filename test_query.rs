fn main() {
    let query = "";
    let server_name = "github";
    let server_matches_query = query.is_empty() || server_name.to_lowercase().contains(&query);
    println!("server_matches_query: {}", server_matches_query);
}
