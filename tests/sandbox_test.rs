mod tests {
    use solarium::sandbox::Sandbox;
    use solana_program::native_token::LAMPORTS_PER_SOL;

    #[test]
    fn start_airdrop_stop() {
        let sandbox = Sandbox::new().unwrap();
        let actor = sandbox.create_actor();
        actor.airdrop(10 * LAMPORTS_PER_SOL);
    }
}
