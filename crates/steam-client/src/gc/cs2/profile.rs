//! What the game coordinator says of a player: ranks, wins, level, commendations, penalties.

use std::convert::TryFrom;
use std::fmt;
use std::time::Duration;

use steam_protobuf::protobufs::csgo::cstrike15_gcmessages::CMsgGCCStrike15_v2_MatchmakingGC2ClientHello;
use steamid_parser::SteamID;

/// The XP the game coordinator counts from, at the start of each level, as node-globaloffensive's README says.
const XP_BASE: i32 = 327_680_000;

/// A player's profile, as the game coordinator gives it: a `CMsgGCCStrike15_v2_MatchmakingGC2ClientHello`, which is
/// both the account's own matchmaking stats (`MatchmakingGC2ClientHello`, 9110) and a player's profile
/// (`PlayersProfile`, 9128).
///
/// What the methods don't give, such as the ongoing match, the tournament or the per-map ranks, is in
/// [`raw`](Self::raw).
#[derive(Clone, PartialEq)]
pub struct PlayerProfile {
    /// Boxed: the events that carry profiles stay small.
    proto: Box<CMsgGCCStrike15_v2_MatchmakingGC2ClientHello>,
}

impl PlayerProfile {
    pub(crate) fn new(proto: CMsgGCCStrike15_v2_MatchmakingGC2ClientHello) -> Self {
        Self { proto: Box::new(proto) }
    }

    /// The player. `None` when the game coordinator doesn't say.
    pub fn player(&self) -> Option<SteamID> {
        self.proto
            .account_id
            .filter(|&id| id != 0)
            .map(SteamID::from_account_id)
    }

    /// The player's ranks: the matchmaking one first (`ranking`), then the others (`rankings`), such as Wingman and
    /// Premier.
    pub fn ranks(&self) -> impl Iterator<Item = Rank> + '_ {
        self.proto
            .ranking
            .as_ref()
            .into_iter()
            .chain(&self.proto.rankings)
            .map(|ranking| Rank {
                rank_type: ranking.rank_type_id,
                rank: ranking.rank_id(),
                wins: ranking.wins(),
            })
    }

    /// The player's level (private rank), from 1. `None` when the game coordinator doesn't say.
    pub fn level(&self) -> Option<u32> {
        self.proto.player_level.and_then(|level| u32::try_from(level).ok())
    }

    /// The XP earned in the current level (5000 a level). `None` when the game coordinator doesn't say.
    pub fn xp(&self) -> Option<u32> {
        let xp = self.proto.player_cur_xp?.checked_sub(XP_BASE)?;
        u32::try_from(xp).ok()
    }

    /// The commendations the player received. `None` when the game coordinator doesn't say.
    pub fn commendations(&self) -> Option<Commendations> {
        self.proto.commendation.as_ref().map(|commendation| Commendations {
            friendly: commendation.cmd_friendly(),
            teaching: commendation.cmd_teaching(),
            leader: commendation.cmd_leader(),
        })
    }

    /// The matchmaking cooldown the player serves, if any.
    pub fn penalty(&self) -> Option<Penalty> {
        let seconds = self.proto.penalty_seconds.filter(|&seconds| seconds != 0)?;
        Some(Penalty {
            remaining: Duration::from_secs(u64::from(seconds)),
            reason: self.proto.penalty_reason(),
        })
    }

    /// Whether the account is VAC banned.
    pub fn vac_banned(&self) -> bool {
        self.proto.vac_banned() != 0
    }

    /// Whether the player is in a match the game coordinator knows of (details in [`raw`](Self::raw)'s
    /// `ongoingmatch`).
    pub fn in_match(&self) -> bool {
        self.proto.ongoingmatch.is_some()
    }

    /// The coins, pins and medals the player shows, as numbered in `items_game.txt`.
    pub fn medals(&self) -> &[u32] {
        self.proto
            .medals
            .as_ref()
            .map_or(&[], |medals| medals.display_items_defidx.as_slice())
    }

    /// The medal the player features, if any.
    pub fn featured_medal(&self) -> Option<u32> {
        self.proto
            .medals
            .as_ref()
            .and_then(|medals| medals.featured_display_item_defidx)
            .filter(|&medal| medal != 0)
    }

    /// The message the game coordinator gave.
    pub fn raw(&self) -> &CMsgGCCStrike15_v2_MatchmakingGC2ClientHello {
        &self.proto
    }

    /// The message the game coordinator gave.
    pub fn into_proto(self) -> CMsgGCCStrike15_v2_MatchmakingGC2ClientHello {
        *self.proto
    }
}

/// Debug shows what the methods give.
impl fmt::Debug for PlayerProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PlayerProfile")
            .field("player", &self.player())
            .field("ranks", &self.ranks().collect::<Vec<_>>())
            .field("level", &self.level())
            .field("xp", &self.xp())
            .field("commendations", &self.commendations())
            .field("penalty", &self.penalty())
            .field("vac_banned", &self.vac_banned())
            .field("in_match", &self.in_match())
            .field("medals", &self.medals())
            .finish()
    }
}

/// A rank of a player in one game mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Rank {
    /// The game mode: 6 for competitive matchmaking, 7 for Wingman, 10 for Danger Zone (node-globaloffensive's
    /// values); CS2's Premier has its own. `None` when the game coordinator doesn't say, as it often doesn't for
    /// the matchmaking rank.
    pub rank_type: Option<u32>,
    /// The rank: 0 for none, then 1 (Silver I) to 18 (The Global Elite) in matchmaking and Wingman; the rating in
    /// Premier.
    pub rank: u32,
    /// The wins in that mode.
    pub wins: u32,
}

/// The commendations a player received.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Commendations {
    /// "Friendly".
    pub friendly: u32,
    /// "Good teacher".
    pub teaching: u32,
    /// "Good leader".
    pub leader: u32,
}

/// A matchmaking cooldown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Penalty {
    /// How long it still lasts, when the game coordinator said.
    pub remaining: Duration,
    /// Why, in the game's own numbering.
    pub reason: u32,
}

#[cfg(test)]
mod tests {
    use steam_protobuf::protobufs::csgo::cstrike15_gcmessages::PlayerCommendationInfo;
    use steam_protobuf::protobufs::csgo::cstrike15_gcmessages::PlayerMedalsInfo;
    use steam_protobuf::protobufs::csgo::cstrike15_gcmessages::PlayerRankingInfo;

    use super::*;

    fn ranking(rank_type: Option<u32>, rank: u32, wins: u32) -> PlayerRankingInfo {
        let mut ranking = PlayerRankingInfo::new();
        ranking.rank_type_id = rank_type;
        ranking.set_rank_id(rank);
        ranking.set_wins(wins);
        ranking
    }

    #[test]
    fn reads_a_profile() {
        let mut proto = CMsgGCCStrike15_v2_MatchmakingGC2ClientHello::new();
        proto.set_account_id(124484118);
        proto.ranking = Some(ranking(None, 15, 302)).into();
        proto.rankings.push(ranking(Some(7), 9, 41));
        proto.rankings.push(ranking(Some(11), 14350, 77));
        proto.set_player_level(23);
        proto.set_player_cur_xp(XP_BASE + 1234);
        let mut commendation = PlayerCommendationInfo::new();
        commendation.set_cmd_friendly(10);
        commendation.set_cmd_teaching(2);
        commendation.set_cmd_leader(3);
        proto.commendation = Some(commendation).into();
        proto.set_penalty_seconds(1800);
        proto.set_penalty_reason(5);
        proto.set_vac_banned(0);
        let mut medals = PlayerMedalsInfo::new();
        medals.display_items_defidx = vec![874, 4555];
        medals.set_featured_display_item_defidx(4555);
        proto.medals = Some(medals).into();

        let profile = PlayerProfile::new(proto);
        assert_eq!(profile.player(), Some(SteamID::from_steam64(76561198084749846)));
        let ranks: Vec<_> = profile
            .ranks()
            .map(|rank| (rank.rank_type, rank.rank, rank.wins))
            .collect();
        assert_eq!(ranks, [(None, 15, 302), (Some(7), 9, 41), (Some(11), 14350, 77)]);
        assert_eq!(profile.level(), Some(23));
        assert_eq!(profile.xp(), Some(1234));
        let commendations = profile.commendations().unwrap();
        assert_eq!(
            (commendations.friendly, commendations.teaching, commendations.leader),
            (10, 2, 3)
        );
        let penalty = profile.penalty().unwrap();
        assert_eq!((penalty.remaining, penalty.reason), (Duration::from_secs(1800), 5));
        assert!(!profile.vac_banned());
        assert!(!profile.in_match());
        assert_eq!(profile.medals(), [874, 4555]);
        assert_eq!(profile.featured_medal(), Some(4555));
        assert!(format!("{:?}", profile).contains("level: Some(23)"));
    }

    #[test]
    fn an_empty_profile_says_nothing() {
        let mut proto = CMsgGCCStrike15_v2_MatchmakingGC2ClientHello::new();
        // below the base, as the game coordinator never sends
        proto.set_player_cur_xp(5);
        proto.set_player_level(-1);
        proto.set_vac_banned(1);
        let profile = PlayerProfile::new(proto);
        assert_eq!(profile.player(), None);
        assert_eq!(profile.ranks().count(), 0);
        assert_eq!(profile.level(), None);
        assert_eq!(profile.xp(), None);
        assert_eq!(profile.commendations(), None);
        assert_eq!(profile.penalty(), None);
        assert!(profile.vac_banned());
        assert!(profile.medals().is_empty());
        assert_eq!(profile.featured_medal(), None);
    }
}
