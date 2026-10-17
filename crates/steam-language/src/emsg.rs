//! [`EMsg`] message ids, and helpers to read them from raw packets.

use arrayref::array_ref;
use num_derive::FromPrimitive;
use num_derive::ToPrimitive;
use num_traits::FromPrimitive;
use serde_repr::Deserialize_repr;
use serde_repr::Serialize_repr;

#[derive(FromPrimitive, ToPrimitive, Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize_repr, Deserialize_repr)]
#[repr(i32)]
#[non_exhaustive]
pub enum EMsg {
    Invalid = 0,
    Multi = 1,
    RemoteSysID = 128,
    FileXferRequest = 1200,
    FileXferResponse = 1201,
    FileXferData = 1202,
    FileXferEnd = 1203,
    FileXferDataAck = 1204,
    ChannelEncryptRequest = 1303,
    ChannelEncryptResponse = 1304,
    ChannelEncryptResult = 1305,
    ClientReportOverlayDetourFailure = 5517,
    ClientMMSGetLobbyData = 6611,
    ClientMMSLobbyData = 6612,
    ClientChatAction = 597,
    CSUserContentRequest = 652,
    ClientLogOn_Deprecated = 701,
    ClientAnonLogOn_Deprecated = 702,
    ClientHeartBeat = 703,
    ClientVACResponse = 704,
    ClientLogOff = 706,
    ClientNoUDPConnectivity = 707,
    ClientInformOfCreateAccount = 708,
    ClientAckVACBan = 709,
    ClientConnectionStats = 710,
    ClientInitPurchase = 711,
    ClientPingResponse = 712,
    ClientRemoveFriend = 714,
    ClientGamesPlayedNoDataBlob = 715,
    ClientChangeStatus = 716,
    ClientVacStatusResponse = 717,
    ClientFriendMsg = 718,
    ClientGetFinalPrice = 722,
    ClientSystemIM = 726,
    ClientSystemIMAck = 727,
    ClientGetLicenses = 728,
    ClientCancelLicense = 729,
    ClientGetLegacyGameKey = 730,
    ClientContentServerLogOn_Deprecated = 731,
    ClientAckVACBan2 = 732,
    ClientCompletePurchase = 733,
    ClientCancelPurchase = 734,
    ClientAckMessageByGID = 735,
    ClientGetPurchaseReceipts = 736,
    ClientAckPurchaseReceipt = 737,
    ClientSendGuestPass = 739,
    ClientAckGuestPass = 740,
    ClientRedeemGuestPass = 741,
    ClientGamesPlayed = 742,
    ClientRegisterKey = 743,
    ClientInviteUserToClan = 744,
    ClientAcknowledgeClanInvite = 745,
    ClientPurchaseWithMachineID = 746,
    ClientAppUsageEvent = 747,
    ClientGetGiftTargetList = 748,
    ClientGetGiftTargetListResponse = 749,
    ClientLogOnResponse = 751,
    ClientVACChallenge = 753,
    ClientSetHeartbeatRate = 755,
    ClientNotLoggedOnDeprecated = 756,
    ClientLoggedOff = 757,
    GSApprove = 758,
    GSDeny = 759,
    GSKick = 760,
    ClientCreateAcctResponse = 761,
    ClientPurchaseResponse = 763,
    ClientPing = 764,
    ClientNOP = 765,
    ClientPersonaState = 766,
    ClientFriendsList = 767,
    ClientAccountInfo = 768,
    ClientVacStatusQuery = 770,
    ClientNewsUpdate = 771,
    ClientGameConnectDeny = 773,
    GSStatusReply = 774,
    ClientGetFinalPriceResponse = 775,
    ClientGameConnectTokens = 779,
    ClientLicenseList = 780,
    ClientCancelLicenseResponse = 781,
    ClientVACBanStatus = 782,
    ClientCMList = 783,
    ClientEncryptPct = 784,
    ClientGetLegacyGameKeyResponse = 785,
    CSUserContentApprove = 787,
    CSUserContentDeny = 788,
    ClientInitPurchaseResponse = 789,
    ClientAddFriend = 791,
    ClientAddFriendResponse = 792,
    ClientInviteFriend = 793,
    ClientInviteFriendResponse = 794,
    ClientSendGuestPassResponse = 795,
    ClientAckGuestPassResponse = 796,
    ClientRedeemGuestPassResponse = 797,
    ClientUpdateGuestPassesList = 798,
    ClientChatMsg = 799,
    ClientChatInvite = 800,
    ClientJoinChat = 801,
    ClientChatMemberInfo = 802,
    ClientLogOnWithCredentials_Deprecated = 803,
    ClientPasswordChangeResponse = 805,
    ClientChatEnter = 807,
    ClientFriendRemovedFromSource = 808,
    ClientCreateChat = 809,
    ClientCreateChatResponse = 810,
    ClientUpdateChatMetadata = 811,
    ClientP2PIntroducerMessage = 813,
    ClientChatActionResult = 814,
    ClientRequestFriendData = 815,
    ClientGetUserStats = 818,
    ClientGetUserStatsResponse = 819,
    ClientStoreUserStats = 820,
    ClientStoreUserStatsResponse = 821,
    ClientClanState = 822,
    ClientServiceModule = 830,
    ClientServiceCall = 831,
    ClientServiceCallResponse = 832,
    ClientNatTraversalStatEvent = 839,
    ClientAppInfoRequest = 840,
    ClientAppInfoResponse = 841,
    ClientSteamUsageEvent = 842,
    ClientCheckPassword = 845,
    ClientResetPassword = 846,
    ClientCheckPasswordResponse = 848,
    ClientResetPasswordResponse = 849,
    ClientSessionToken = 850,
    ClientDRMProblemReport = 851,
    ClientSetIgnoreFriend = 855,
    ClientSetIgnoreFriendResponse = 856,
    ClientGetAppOwnershipTicket = 857,
    ClientGetAppOwnershipTicketResponse = 858,
    ClientGetLobbyListResponse = 860,
    ClientGetLobbyMetadata = 861,
    ClientGetLobbyMetadataResponse = 862,
    ClientVTTCert = 863,
    ClientAppInfoUpdate = 866,
    ClientAppInfoChanges = 867,
    ClientServerList = 880,
    ClientGetFriendsLobbies = 888,
    ClientGetFriendsLobbiesResponse = 889,
    ClientGetLobbyList = 890,
    ClientEmailChangeResponse = 891,
    ClientSecretQAChangeResponse = 892,
    ClientDRMBlobRequest = 896,
    ClientDRMBlobResponse = 897,
    ClientLookupKey = 898,
    ClientLookupKeyResponse = 899,
    GSDisconnectNotice = 901,
    GSStatus = 903,
    GSUserPlaying = 905,
    GSStatus2 = 906,
    GSStatusUpdate_Unused = 907,
    GSServerType = 908,
    GSPlayerList = 909,
    GSGetUserAchievementStatus = 910,
    GSGetUserAchievementStatusResponse = 911,
    GSGetPlayStats = 918,
    GSGetPlayStatsResponse = 919,
    GSGetUserGroupStatus = 920,
    GSGetUserGroupStatusResponse = 923,
    GSGetReputation = 936,
    GSGetReputationResponse = 937,
    ClientChatRoomInfo = 4026,
    ClientUFSUploadFileRequest = 5202,
    ClientUFSUploadFileResponse = 5203,
    ClientUFSUploadFileChunk = 5204,
    ClientUFSUploadFileFinished = 5205,
    ClientUFSGetFileListForApp = 5206,
    ClientUFSGetFileListForAppResponse = 5207,
    ClientUFSDownloadRequest = 5210,
    ClientUFSDownloadResponse = 5211,
    ClientUFSDownloadChunk = 5212,
    ClientUFSLoginRequest = 5213,
    ClientUFSLoginResponse = 5214,
    ClientUFSTransferHeartbeat = 5216,
    ClientUFSDeleteFileRequest = 5219,
    ClientUFSDeleteFileResponse = 5220,
    ClientUFSGetUGCDetails = 5226,
    ClientUFSGetUGCDetailsResponse = 5227,
    ClientUFSGetSingleFileInfo = 5230,
    ClientUFSGetSingleFileInfoResponse = 5231,
    ClientUFSShareFile = 5232,
    ClientUFSShareFileResponse = 5233,
    ClientRequestForgottenPasswordEmail = 5401,
    ClientRequestForgottenPasswordEmailResponse = 5402,
    ClientCreateAccountResponse = 5403,
    ClientResetForgottenPassword = 5404,
    ClientResetForgottenPasswordResponse = 5405,
    ClientCreateAccount2 = 5406,
    ClientInformOfResetForgottenPassword = 5407,
    ClientInformOfResetForgottenPasswordResponse = 5408,
    ClientAnonUserLogOn_Deprecated = 5409,
    ClientGamesPlayedWithDataBlob = 5410,
    ClientUpdateUserGameInfo = 5411,
    ClientFileToDownload = 5412,
    ClientFileToDownloadResponse = 5413,
    ClientLBSSetScore = 5414,
    ClientLBSSetScoreResponse = 5415,
    ClientLBSFindOrCreateLB = 5416,
    ClientLBSFindOrCreateLBResponse = 5417,
    ClientLBSGetLBEntries = 5418,
    ClientLBSGetLBEntriesResponse = 5419,
    ClientMarketingMessageUpdate = 5420,
    ClientChatDeclined = 5426,
    ClientFriendMsgIncoming = 5427,
    ClientAuthList_Deprecated = 5428,
    ClientTicketAuthComplete = 5429,
    ClientIsLimitedAccount = 5430,
    ClientAuthList = 5432,
    ClientStat = 5433,
    ClientP2PConnectionInfo = 5434,
    ClientP2PConnectionFailInfo = 5435,
    ClientGetNumberOfCurrentPlayers = 5436,
    ClientGetNumberOfCurrentPlayersResponse = 5437,
    ClientGetDepotDecryptionKey = 5438,
    ClientGetDepotDecryptionKeyResponse = 5439,
    GSPerformHardwareSurvey = 5440,
    ClientEnableTestLicense = 5443,
    ClientEnableTestLicenseResponse = 5444,
    ClientDisableTestLicense = 5445,
    ClientDisableTestLicenseResponse = 5446,
    ClientRequestValidationMail = 5448,
    ClientRequestValidationMailResponse = 5449,
    ClientToGC = 5452,
    ClientFromGC = 5453,
    ClientRequestChangeMail = 5454,
    ClientRequestChangeMailResponse = 5455,
    ClientEmailAddrInfo = 5456,
    ClientPasswordChange3 = 5457,
    ClientEmailChange3 = 5458,
    ClientPersonalQAChange3 = 5459,
    ClientResetForgottenPassword3 = 5460,
    ClientRequestForgottenPasswordEmail3 = 5461,
    ClientCreateAccount3 = 5462,
    ClientNewLoginKey = 5463,
    ClientNewLoginKeyAccepted = 5464,
    ClientLogOnWithHash_Deprecated = 5465,
    ClientStoreUserStats2 = 5466,
    ClientStatsUpdated = 5467,
    ClientActivateOEMLicense = 5468,
    ClientRequestedClientStats = 5480,
    ClientStat2Int32 = 5481,
    ClientStat2 = 5482,
    ClientVerifyPassword = 5483,
    ClientVerifyPasswordResponse = 5484,
    ClientDRMDownloadRequest = 5485,
    ClientDRMDownloadResponse = 5486,
    ClientDRMFinalResult = 5487,
    ClientGetFriendsWhoPlayGame = 5488,
    ClientGetFriendsWhoPlayGameResponse = 5489,
    ClientOGSBeginSession = 5490,
    ClientOGSBeginSessionResponse = 5491,
    ClientOGSEndSession = 5492,
    ClientOGSEndSessionResponse = 5493,
    ClientOGSWriteRow = 5494,
    ClientDRMTest = 5495,
    ClientDRMTestResult = 5496,
    ClientServerUnavailable = 5500,
    ClientServersAvailable = 5501,
    ClientRegisterAuthTicketWithCM = 5502,
    ClientGCMsgFailed = 5503,
    ClientMicroTxnAuthRequest = 5504,
    ClientMicroTxnAuthorize = 5505,
    ClientMicroTxnAuthorizeResponse = 5506,
    ClientAppMinutesPlayedData = 5507,
    ClientGetMicroTxnInfo = 5508,
    ClientGetMicroTxnInfoResponse = 5509,
    ClientMarketingMessageUpdate2 = 5510,
    ClientDeregisterWithServer = 5511,
    ClientSubscribeToPersonaFeed = 5512,
    ClientLogon = 5514,
    ClientGetClientDetails = 5515,
    ClientGetClientDetailsResponse = 5516,
    ClientGetClientAppList = 5518,
    ClientGetClientAppListResponse = 5519,
    ClientInstallClientApp = 5520,
    ClientInstallClientAppResponse = 5521,
    ClientUninstallClientApp = 5522,
    ClientUninstallClientAppResponse = 5523,
    ClientSetClientAppUpdateState = 5524,
    ClientSetClientAppUpdateStateResponse = 5525,
    ClientRequestEncryptedAppTicket = 5526,
    ClientRequestEncryptedAppTicketResponse = 5527,
    ClientWalletInfoUpdate = 5528,
    ClientLBSSetUGC = 5529,
    ClientLBSSetUGCResponse = 5530,
    ClientAMGetClanOfficers = 5531,
    ClientAMGetClanOfficersResponse = 5532,
    ClientCheckFileSignature = 5533,
    ClientCheckFileSignatureResponse = 5534,
    ClientFriendProfileInfo = 5535,
    ClientFriendProfileInfoResponse = 5536,
    ClientUpdateMachineAuth = 5537,
    ClientUpdateMachineAuthResponse = 5538,
    ClientReadMachineAuth = 5539,
    ClientReadMachineAuthResponse = 5540,
    ClientRequestMachineAuth = 5541,
    ClientRequestMachineAuthResponse = 5542,
    ClientScreenshotsChanged = 5543,
    ClientEmailChange4 = 5544,
    ClientEmailChangeResponse4 = 5545,
    ClientDFSAuthenticateRequest = 5605,
    ClientDFSAuthenticateResponse = 5606,
    ClientDFSEndSession = 5607,
    ClientDFSDownloadStatus = 5617,
    ClientMDSLoginRequest = 5801,
    ClientMDSLoginResponse = 5802,
    ClientMDSUploadManifestRequest = 5803,
    ClientMDSUploadManifestResponse = 5804,
    ClientMDSTransmitManifestDataChunk = 5805,
    ClientMDSHeartbeat = 5806,
    ClientMDSUploadDepotChunks = 5807,
    ClientMDSUploadDepotChunksResponse = 5808,
    ClientMDSInitDepotBuildRequest = 5809,
    ClientMDSInitDepotBuildResponse = 5810,
    ClientMDSGetDepotManifest = 5818,
    ClientMDSGetDepotManifestResponse = 5819,
    ClientMDSGetDepotManifestChunk = 5820,
    ClientMDSDownloadDepotChunksRequest = 5823,
    ClientMDSDownloadDepotChunksAsync = 5824,
    ClientMDSDownloadDepotChunksAck = 5825,
    ClientMMSCreateLobby = 6601,
    ClientMMSCreateLobbyResponse = 6602,
    ClientMMSJoinLobby = 6603,
    ClientMMSJoinLobbyResponse = 6604,
    ClientMMSLeaveLobby = 6605,
    ClientMMSLeaveLobbyResponse = 6606,
    ClientMMSGetLobbyList = 6607,
    ClientMMSGetLobbyListResponse = 6608,
    ClientMMSSetLobbyData = 6609,
    ClientMMSSetLobbyDataResponse = 6610,
    ClientMMSSendLobbyChatMsg = 6613,
    ClientMMSLobbyChatMsg = 6614,
    ClientMMSSetLobbyOwner = 6615,
    ClientMMSSetLobbyOwnerResponse = 6616,
    ClientMMSSetLobbyGameServer = 6617,
    ClientMMSLobbyGameServerSet = 6618,
    ClientMMSUserJoinedLobby = 6619,
    ClientMMSUserLeftLobby = 6620,
    ClientMMSInviteToLobby = 6621,
    ClientUDSP2PSessionStarted = 7001,
    ClientUDSP2PSessionEnded = 7002,
    ClientUDSInviteToGame = 7005,
    ClientUCMAddScreenshot = 7301,
    ClientUCMAddScreenshotResponse = 7302,
    ClientUCMGetScreenshotList = 7305,
    ClientUCMGetScreenshotListResponse = 7306,
    ClientUCMDeleteScreenshot = 7309,
    ClientUCMDeleteScreenshotResponse = 7310,
    ClientRichPresenceUpload = 7501,
    ClientRichPresenceRequest = 7502,
    ClientRichPresenceInfo = 7503,
}

/// Set on the raw EMsg of messages with a protobuf header.
pub(crate) const PROTOMASK: u32 = 0x8000_0000;
const EMSGMASK: u32 = !PROTOMASK;

impl EMsg {
    /// Creates an `EMsg` from a uint32
    ///
    /// # Panics
    ///
    /// If the id is unknown. Steam sends ids that are not listed here, so prefer `EMsg::from_u32`, from
    /// [`FromPrimitive`], on data from the network.
    pub fn from_uint(message: u32) -> Self {
        match EMsg::from_u32(message) {
            Some(value) => value,
            None => panic!("ABORT"),
        }
    }

    /// Creates an `EMsg` from a raw data packet, ignoring its protobuf flag.
    pub fn from_raw_message(message: &[u8]) -> Result<Self, EMsgError> {
        Self::split_raw(message).map(|(emsg, _, _)| emsg)
    }

    /// Splits a raw data packet into its `EMsg`, whether it is flagged as a protobuf, and the data after it.
    pub fn split_raw(message: &[u8]) -> Result<(Self, bool, &[u8]), EMsgError> {
        if message.len() < 4 {
            return Err(EMsgError::MessageNotLongEnough("a message needs 4 bytes for its EMsg"));
        }
        let raw = Self::extract_varint(message);

        match EMsg::from_u32(raw & EMSGMASK) {
            Some(value) => Ok((value, raw & PROTOMASK != 0, &message[4..])),
            None => Err(EMsgError::ValueNotFound("the EMsg is unknown")),
        }
    }

    /// Returns only the protobuf flag of a raw `EMsg`, which is zero when it is not set.
    pub fn strip_protobuf_flag(message: u32) -> u32 {
        message & PROTOMASK
    }

    /// Strips the [EMsg] from data, and returns the data
    ///
    /// # Panics
    ///
    /// If `message` is shorter than 4 bytes. [`EMsg::split_raw`] returns an error instead.
    pub fn strip_message(message: &[u8]) -> &[u8] {
        &message[4..]
    }

    /// Checks if a message is flagged as a protobuf
    /// We can only check with the varint on it
    ///
    /// # Panics
    ///
    /// If `message` is shorter than 4 bytes. [`EMsg::split_raw`] returns an error instead.
    pub fn is_protobuf(message: &[u8]) -> bool {
        Self::strip_protobuf_flag(Self::extract_varint(message)) > 0
    }

    /// Extract varint from data
    ///
    /// # Panics
    ///
    /// If `message` is shorter than 4 bytes.
    pub fn extract_varint(message: &[u8]) -> u32 {
        u32::from_le_bytes(*array_ref!(message, 0, 4))
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum EMsgError {
    MessageNotLongEnough(&'static str),
    ValueNotFound(&'static str),
}

impl std::fmt::Display for EMsgError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EMsgError::MessageNotLongEnough(reason) | EMsgError::ValueNotFound(reason) => f.write_str(reason),
        }
    }
}

impl std::error::Error for EMsgError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_raw_masks_the_protobuf_flag() {
        let raw = (EMsg::ClientLogon as u32 | PROTOMASK).to_le_bytes();
        let (emsg, is_protobuf, rest) = EMsg::split_raw(&raw).unwrap();

        assert_eq!(emsg, EMsg::ClientLogon);
        assert!(is_protobuf);
        assert!(rest.is_empty());
        assert_eq!(EMsg::from_raw_message(&raw).unwrap(), EMsg::ClientLogon);
    }

    #[test]
    fn split_raw_without_the_protobuf_flag() {
        let raw = [0x17, 0x05, 0x00, 0x00, 0xAA];
        let (emsg, is_protobuf, rest) = EMsg::split_raw(&raw).unwrap();

        assert_eq!(emsg, EMsg::ChannelEncryptRequest);
        assert!(!is_protobuf);
        assert_eq!(rest, [0xAA]);
    }

    #[test]
    fn split_raw_rejects_short_and_unknown_input() {
        for len in 0..4 {
            assert!(matches!(
                EMsg::split_raw(&[0x17, 0x05, 0x00, 0x00][..len]),
                Err(EMsgError::MessageNotLongEnough(_))
            ));
        }
        assert!(matches!(
            EMsg::from_raw_message(&u32::MAX.to_le_bytes()),
            Err(EMsgError::ValueNotFound(_))
        ));
    }

}
