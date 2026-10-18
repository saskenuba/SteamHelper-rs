//! Matches, as the game coordinator lists them: a match's details from its share code, a player's recent matches,
//! the live ones.

use std::convert::TryFrom;
use std::fmt;
use std::slice;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use steam_protobuf::protobufs::csgo::cstrike15_gcmessages::CDataGCCStrike15_v2_MatchInfo;
use steam_protobuf::protobufs::csgo::cstrike15_gcmessages::CMsgGCCStrike15_v2_MatchList;
use steam_protobuf::protobufs::csgo::cstrike15_gcmessages::CMsgGCCStrike15_v2_MatchmakingServerRoundStats;
use steamid_parser::SteamID;

use crate::gc::cs2::ShareCode;

/// The matches the game coordinator gives (`MatchList`, 9139), in answer to a request about matches.
///
/// What the methods don't give, such as the tournament, is in [`raw`](Self::raw).
#[derive(Clone, PartialEq)]
pub struct MatchList {
    proto: CMsgGCCStrike15_v2_MatchList,
}

impl MatchList {
    pub(crate) fn new(proto: CMsgGCCStrike15_v2_MatchList) -> Self {
        Self { proto }
    }

    /// The matches. None when the game coordinator found none.
    pub fn matches(&self) -> impl ExactSizeIterator<Item = MatchInfo<'_>> {
        self.proto.matches.iter().map(|proto| MatchInfo { proto })
    }

    /// The player the list is about, for a player's recent or live matches.
    pub fn player(&self) -> Option<SteamID> {
        self.proto.accountid.filter(|&id| id != 0).map(SteamID::from_account_id)
    }

    /// The message the game coordinator gave.
    pub fn raw(&self) -> &CMsgGCCStrike15_v2_MatchList {
        &self.proto
    }

    /// The message the game coordinator gave.
    pub fn into_proto(self) -> CMsgGCCStrike15_v2_MatchList {
        self.proto
    }
}

/// Debug shows what the methods give.
impl fmt::Debug for MatchList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MatchList")
            .field("player", &self.player())
            .field("matches", &self.matches().collect::<Vec<_>>())
            .finish()
    }
}

/// A match of a [`MatchList`]: a `CDataGCCStrike15_v2_MatchInfo`. Its rounds' stats are cumulative: the last round
/// has the match's totals.
#[derive(Clone, Copy, PartialEq)]
pub struct MatchInfo<'a> {
    proto: &'a CDataGCCStrike15_v2_MatchInfo,
}

impl<'a> MatchInfo<'a> {
    /// The match's id. `None` when the game coordinator doesn't say.
    pub fn match_id(&self) -> Option<u64> {
        self.proto.matchid.filter(|&id| id != 0)
    }

    /// When the match was played.
    pub fn time(&self) -> Option<SystemTime> {
        self.proto
            .matchtime
            .filter(|&time| time != 0)
            .map(|time| UNIX_EPOCH + Duration::from_secs(u64::from(time)))
    }

    /// The map, such as `de_mirage`, when the game coordinator says.
    pub fn map(&self) -> Option<&'a str> {
        self.proto
            .watchablematchinfo
            .as_ref()
            .and_then(|watchable| watchable.game_map.as_deref())
            .filter(|map| !map.is_empty())
    }

    /// The stats after each round the game coordinator gives, the last one with the match's totals.
    pub fn rounds(&self) -> &'a [CMsgGCCStrike15_v2_MatchmakingServerRoundStats] {
        match self.proto.roundstats_legacy.as_ref() {
            Some(legacy) if self.proto.roundstatsall.is_empty() => slice::from_ref(legacy),
            _ => &self.proto.roundstatsall,
        }
    }

    /// The stats at the end of the match (or the latest round, for a live one).
    pub fn last_round(&self) -> Option<&'a CMsgGCCStrike15_v2_MatchmakingServerRoundStats> {
        self.rounds().last()
    }

    /// Where the match's demo can be downloaded (`.dem.bz2`), while Valve keeps it: the last round's `map`, which
    /// holds a URL for a finished match.
    pub fn demo_url(&self) -> Option<&'a str> {
        self.last_round()
            .and_then(|round| round.map.as_deref())
            .filter(|url| url.starts_with("http://") || url.starts_with("https://"))
    }

    /// The teams' scores, as the last round has them.
    pub fn score(&self) -> Option<[i32; 2]> {
        match self.last_round()?.team_scores.as_slice() {
            &[first, second] => Some([first, second]),
            _ => None,
        }
    }

    /// How long the match lasted (so far, for a live one).
    pub fn duration(&self) -> Option<Duration> {
        let seconds = self.last_round()?.match_duration?;
        u64::try_from(seconds).ok().map(Duration::from_secs)
    }

    /// The players, in the order of the rounds' per-player stats (`kills`, `deaths`…).
    pub fn players(&self) -> impl ExactSizeIterator<Item = SteamID> + 'a {
        self.last_round()
            .and_then(|round| round.reservation.as_ref())
            .map_or(&[][..], |reservation| reservation.account_ids.as_slice())
            .iter()
            .map(|&id| SteamID::from_account_id(id))
    }

    /// The match's share code, made from its id, its last round's reservation id and its GOTV port, as akiver's
    /// csgo-sharecode `encodeMatch` does.
    pub fn share_code(&self) -> Option<ShareCode> {
        let match_id = self.match_id()?;
        let outcome_id = self.last_round()?.reservationid?;
        let port = self.proto.watchablematchinfo.as_ref()?.tv_port?;
        Some(ShareCode::new(match_id, outcome_id, u16::try_from(port).ok()?))
    }

    /// The message the game coordinator gave.
    pub fn raw(&self) -> &'a CDataGCCStrike15_v2_MatchInfo {
        self.proto
    }
}

/// Debug shows what the methods give.
impl fmt::Debug for MatchInfo<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MatchInfo")
            .field("match_id", &self.match_id())
            .field("time", &self.time())
            .field("map", &self.map())
            .field("rounds", &self.rounds().len())
            .field("score", &self.score())
            .field("duration", &self.duration())
            .field("demo_url", &self.demo_url())
            .field("share_code", &self.share_code())
            .finish()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use steam_protobuf::protobufs::csgo::cstrike15_gcmessages::CMsgGCCStrike15_v2_MatchmakingGC2ServerReserve;
    use steam_protobuf::protobufs::csgo::cstrike15_gcmessages::WatchableMatchInfo;

    use super::*;

    const DEMO: &str = "http://replay129.valve.net/730/003400360672356205056_0123456789.dem.bz2";

    /// A finished match: the one of akiver's first share code sample, 16 to 12 on Mirage.
    pub(crate) fn finished(match_id: u64) -> CDataGCCStrike15_v2_MatchInfo {
        let mut first = CMsgGCCStrike15_v2_MatchmakingServerRoundStats::new();
        first.set_reservationid(3400367402569957763);
        first.team_scores = vec![1, 0];
        let mut last = CMsgGCCStrike15_v2_MatchmakingServerRoundStats::new();
        last.set_reservationid(3400367402569957763);
        last.set_map(DEMO.to_string());
        last.team_scores = vec![16, 12];
        last.set_match_duration(2400);
        let mut reservation = CMsgGCCStrike15_v2_MatchmakingGC2ServerReserve::new();
        reservation.account_ids = vec![124484118, 1];
        last.reservation = Some(reservation).into();
        let mut watchable = WatchableMatchInfo::new();
        watchable.set_tv_port(9725);
        watchable.set_game_map("de_mirage".to_string());
        let mut info = CDataGCCStrike15_v2_MatchInfo::new();
        info.set_matchid(match_id);
        info.set_matchtime(1_700_000_000);
        info.watchablematchinfo = Some(watchable).into();
        info.roundstatsall = vec![first, last];
        info
    }

    #[test]
    fn reads_a_finished_match() {
        let mut proto = CMsgGCCStrike15_v2_MatchList::new();
        proto.set_accountid(124484118);
        proto.matches.push(finished(3400360672356205056));
        let list = MatchList::new(proto);
        assert_eq!(list.player(), Some(SteamID::from_steam64(76561198084749846)));
        assert_eq!(list.matches().len(), 1);
        let info = list.matches().next().unwrap();
        assert_eq!(info.match_id(), Some(3400360672356205056));
        assert_eq!(info.time(), Some(UNIX_EPOCH + Duration::from_secs(1_700_000_000)));
        assert_eq!(info.map(), Some("de_mirage"));
        assert_eq!(info.rounds().len(), 2);
        assert_eq!(info.score(), Some([16, 12]));
        assert_eq!(info.duration(), Some(Duration::from_secs(2400)));
        assert_eq!(info.demo_url(), Some(DEMO));
        let players: Vec<_> = info.players().map(SteamID::account_id).collect();
        assert_eq!(players, [124484118, 1]);
        assert_eq!(
            info.share_code().unwrap().to_string(),
            "CSGO-L9spZ-ihuov-cyhtE-kxbqa-FkBAA"
        );
        assert!(format!("{:?}", list).contains("de_mirage"));
    }

    #[test]
    fn reads_a_match_with_little() {
        let mut info = CDataGCCStrike15_v2_MatchInfo::new();
        let empty = MatchInfo { proto: &info };
        assert_eq!(empty.match_id(), None);
        assert_eq!(empty.time(), None);
        assert_eq!(empty.map(), None);
        assert!(empty.rounds().is_empty());
        assert_eq!(empty.score(), None);
        assert_eq!(empty.duration(), None);
        assert_eq!(empty.demo_url(), None);
        assert_eq!(empty.players().len(), 0);
        assert_eq!(empty.share_code(), None);

        // a live match: the legacy round stats, a map name where finished ones have the demo
        let mut round = CMsgGCCStrike15_v2_MatchmakingServerRoundStats::new();
        round.set_map("de_inferno".to_string());
        round.team_scores = vec![3];
        info.roundstats_legacy = Some(round).into();
        let live = MatchInfo { proto: &info };
        assert_eq!(live.rounds().len(), 1);
        assert_eq!(live.demo_url(), None);
        assert_eq!(live.score(), None);
    }
}
