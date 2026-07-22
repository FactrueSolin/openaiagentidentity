use std::path::Path;

use agentidentity::{
    build_identity_document, build_registration_client, generate_key_material, output_filename,
    parse_account_claims, read_access_token, register_with_openai, write_identity_file,
};
use anyhow::Result;

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let (client, proxy_description) = build_registration_client()?;
    println!("{proxy_description}");

    let token = {
        let stdin = std::io::stdin();
        let mut input = stdin.lock();
        let stdout = std::io::stdout();
        let mut output = stdout.lock();
        read_access_token(&mut input, &mut output)?
    };

    let claims = parse_account_claims(&token)?;
    let key_material = generate_key_material()?;

    println!("Registering Agent Identity with OpenAI...");
    let runtime_id = register_with_openai(&client, &token, claims.is_fedramp, &key_material)?;
    let document = build_identity_document(&runtime_id, &key_material, &claims);
    let filename = output_filename(&claims.email, &claims.plan_type);
    write_identity_file(Path::new(&filename), &document)?;

    println!("Created {filename}");
    Ok(())
}
