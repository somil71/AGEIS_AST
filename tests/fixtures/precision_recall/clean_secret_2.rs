fn get_stripe_key() -> String {
    std::env::var("STRIPE_KEY").unwrap()
}