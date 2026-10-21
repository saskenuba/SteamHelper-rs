/// Where items are kept: a game and one of its inventory contexts. Steam addresses an inventory, and every item in an
/// offer, by this pair (`appid`, `contextid`).
///
/// The games this crate knows are constants; name another game's context with [`InventoryContext::new`]:
///
/// ```
/// use steam_trading::InventoryContext;
///
/// const RUST: InventoryContext = InventoryContext::new(252490, 2);
///
/// assert_eq!(RUST.appid(), 252490);
/// assert_ne!(RUST, InventoryContext::CS2);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InventoryContext {
    appid: u32,
    contextid: u32,
}

impl InventoryContext {
    /// CS2's items that can be traded now.
    pub const CS2: Self = Self::new(730, 2);
    /// CS2's trade-protected items, where received items land (seen live 2026-10-20).
    pub const CS2_TRADE_PROTECTED: Self = Self::new(730, 16);
    /// Dota 2's items.
    pub const DOTA2: Self = Self::new(570, 2);
    /// Team Fortress 2's items.
    pub const TF2: Self = Self::new(440, 2);
    /// Steam's own community items: trading cards, backgrounds, emoticons.
    pub const STEAM: Self = Self::new(753, 6);

    /// The context `contextid` of the game `appid`.
    pub const fn new(appid: u32, contextid: u32) -> Self {
        Self { appid, contextid }
    }

    /// Steam's `appid` of the game.
    pub const fn appid(self) -> u32 {
        self.appid
    }

    /// Steam's `contextid` within the game.
    pub const fn contextid(self) -> u32 {
        self.contextid
    }
}
