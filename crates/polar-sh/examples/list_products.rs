use polar_sh::Resources;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let token = std::env::var("POLAR_ACCESS_TOKEN").expect("POLAR_ACCESS_TOKEN must be set");

    let polar = polar_sh::Polar::new(token);

    let params = polar_sh::operations::ProductsListParams {
        limit: Some(10),
        ..Default::default()
    };

    let page = polar.products().list(&params).await?;
    println!("{} product(s):", page.items.len());
    for p in &page.items {
        println!("  {} — {}", p.id, p.name);
    }

    Ok(())
}
