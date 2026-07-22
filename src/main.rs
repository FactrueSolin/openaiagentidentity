use std::path::Path;

use agentidentity::{
    build_identity_document, build_registration_client, generate_key_material, output_filename,
    parse_account_claims, register_with_openai, write_identity_file,
};
use anyhow::{Context, Result};
use zeroize::Zeroizing;

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let token = Zeroizing::new(
        rpassword::prompt_password("Authentication token: ")
            .context("failed to read authentication token")?,
    );
    let token = token.trim();
    anyhow::ensure!(!token.is_empty(), "authentication token cannot be empty");

    let claims = parse_account_claims(token)?;
    let key_material = generate_key_material()?;
    let client = build_registration_client()?;

    println!("Registering Agent Identity with OpenAI...");
    let runtime_id = register_with_openai(&client, token, claims.is_fedramp, &key_material)?;
    let document = build_identity_document(&runtime_id, &key_material, &claims);
    let filename = output_filename(&claims.email, &claims.plan_type);
    write_identity_file(Path::new(&filename), &document)?;

    println!("Created {filename}");
    Ok(())
}
