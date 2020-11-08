use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize_repr, Deserialize_repr)]
#[repr(i32)]
pub enum EUniverse {
	Invalid = 0,
	Public = 1,
	Beta = 2,
	Internal = 3,
	Dev = 4,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EChatEntryType {
	Invalid = 0,
	ChatMsg = 1,
	Typing = 2,
	InviteGame = 3,
	LeftConversation = 6,
	Entered = 7,
	WasKicked = 8,
	WasBanned = 9,
	Disconnected = 10,
	HistoricalChat = 11,
	Reserved1 = 12,
	Reserved2 = 13,
	LinkBlocked = 14,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EPersonaState {
	Offline = 0,
	Online = 1,
	Busy = 2,
	Away = 3,
	Snooze = 4,
	LookingToTrade = 5,
	LookingToPlay = 6,
	Invisible = 7,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize_repr, Deserialize_repr)]
#[repr(i32)]
pub enum EAccountType {
	Invalid = 0,
	Individual = 1,
	Multiseat = 2,
	GameServer = 3,
	AnonGameServer = 4,
	Pending = 5,
	ContentServer = 6,
	Clan = 7,
	Chat = 8,
	ConsoleUser = 9,
	AnonUser = 10,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EFriendRelationship {
	None = 0,
	Blocked = 1,
	RequestRecipient = 2,
	Friend = 3,
	RequestInitiator = 4,
	Ignored = 5,
	IgnoredFriend = 6,
}

bitflags! {
	#[derive(Serialize, Deserialize)]
	#[allow(non_upper_case_globals)]
	pub struct EAccountFlags: i32 {
		const NormalUser = 0;
		const PersonaNameSet = 1;
		const Unbannable = 2;
		const PasswordSet = 4;
		const Support = 8;
		const Admin = 16;
		const Supervisor = 32;
		const AppEditor = 64;
		const HWIDSet = 128;
		const PersonalQASet = 256;
		const VacBeta = 512;
		const Debug = 1024;
		const Disabled = 2048;
		const LimitedUser = 4096;
		const LimitedUserForce = 8192;
		const EmailValidated = 16384;
		const MarketingTreatment = 32768;
		const OGGInviteOptOut = 65536;
		const ForcePasswordChange = 131072;
		const ForceEmailVerification = 262144;
		const LogonExtraSecurity = 524288;
		const LogonExtraSecurityDisabled = 1048576;
		const Steam2MigrationComplete = 2097152;
		const NeedLogs = 4194304;
		const Lockdown = 8388608;
		const MasterAppEditor = 16777216;
		const BannedFromWebAPI = 33554432;
		const ClansOnlyFromFriends = 67108864;
		const GlobalModerator = 134217728;
		const ParentalSettings = 268435456;
		const ThirdPartySupport = 536870912;
		const NeedsSSANextSteamLogon = 1073741824;
	}
}

bitflags! {
	#[derive(Serialize, Deserialize)]
	pub struct EClanPermission: i32 {
		const Nobody = 0;
		const Owner = 1;
		const Officer = 2;
		const OwnerAndOfficer = 3;
		const Member = 4;
		const Moderator = 8;
		const OwnerOfficerModerator  = Self::Owner.bits | Self::Officer.bits | Self::Moderator.bits;
		const AllMembers  = Self::Owner.bits | Self::Officer.bits | Self::Moderator.bits | Self::Member .bits;
		const OGGGameOwner = 16;
		const NonMember = 128;
		const MemberAllowed		 = Self::NonMember.bits | Self::Member.bits;
		const ModeratorAllowed	 = Self::NonMember.bits | Self::Member.bits | Self::Moderator.bits;
		const OfficerAllowed		 = Self::NonMember.bits | Self::Member.bits | Self::Moderator.bits | Self::Officer.bits;
		const OwnerAllowed		 = Self::NonMember.bits | Self::Member.bits | Self::Moderator.bits | Self::Officer.bits | Self::Owner.bits;
		const Anybody				 = Self::NonMember.bits | Self::Member.bits | Self::Moderator.bits | Self::Officer.bits | Self::Owner.bits;
	}
}

bitflags! {
	#[derive(Serialize, Deserialize)]
	pub struct EChatPermission: i32 {
		const Close = 1;
		const Invite = 2;
		const Talk = 8;
		const Kick = 16;
		const Mute = 32;
		const SetMetadata = 64;
		const ChangePermissions = 128;
		const Ban = 256;
		const ChangeAccess = 512;
//		const EveryoneNotInClanDefault = Self::Talk;
		const EveryoneDefault  = Self::Talk.bits | Self::Invite.bits;
		const MemberDefault  = Self::Ban.bits | Self::Kick.bits | Self::Talk.bits | Self::Invite.bits;
		const OfficerDefault  = Self::Ban.bits | Self::Kick.bits | Self::Talk.bits | Self::Invite.bits;
		const OwnerDefault  = Self::ChangeAccess.bits | Self::Ban.bits | Self::SetMetadata.bits | Self::Mute.bits | Self::Kick.bits | Self::Talk.bits | Self::Invite.bits | Self::Close.bits;
		const Mask = 1019;
	}
}

bitflags! {
	#[derive(Serialize, Deserialize)]
	pub struct EFriendFlags: i32 {
		const None = 0;
		const Blocked = 1;
		const FriendshipRequested = 2;
		const Immediate = 4;
		const ClanMember = 8;
		const OnGameServer = 16;
		const RequestingFriendship = 128;
		const RequestingInfo = 256;
		const Ignored = 512;
		const IgnoredFriend = 1024;
		const Suggested = 2048;
		const ChatMember = 4096;
		const FlagAll = 65535;
	}
}

bitflags! {
	#[derive(Serialize, Deserialize)]
	pub struct EPersonaStateFlag: i32 {
		const HasRichPresence = 1;
		const InJoinableGame = 2;
		const Golden = 4;
		const ClientTypeWeb = 256;
		const ClientTypeMobile = 512;
		const ClientTypeTenfoot = 1024;
		const ClientTypeVR = 2048;
		const LaunchTypeGamepad = 4096;
		const LaunchTypeCompatTool = 8192;
	}
}

bitflags! {
	#[derive(Serialize, Deserialize)]
	pub struct EClientPersonaStateFlag: i32 {
		const Status = 1;
		const PlayerName = 2;
		const QueryPort = 4;
		const SourceID = 8;
		const Presence = 16;
		const LastSeen = 64;
		const UserClanRank = 128;
		const GameExtraInfo = 256;
		const GameDataBlob = 512;
		const ClanData = 1024;
		const Facebook = 2048;
		const RichPresence = 4096;
		const Broadcast = 8192;
		const Watching = 16384;
	}
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EAppUsageEvent {
	GameLaunch = 1,
	GameLaunchTrial = 2,
	Media = 3,
	PreloadStart = 4,
	PreloadFinish = 5,
	MarketingMessageView = 6,
	InGameAdViewed = 7,
	GameLaunchFreeWeekend = 8,
}

bitflags! {
	#[derive(Serialize, Deserialize)]
	pub struct ELicenseFlags: i32 {
		const None = 0;
		const Renew = 0x01;
		const RenewalFailed = 0x02;
		const Pending = 0x04;
		const Expired = 0x08;
		const CancelledByUser = 0x10;
		const CancelledByAdmin = 0x20;
		const LowViolenceContent = 0x40;
		const ImportedFromSteam2 = 0x80;
		const ForceRunRestriction = 0x100;
		const RegionRestrictionExpired = 0x200;
		const CancelledByFriendlyFraudLock = 0x400;
		const NotActivated = 0x800;
	}
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum ELicenseType {
	NoLicense = 0,
	SinglePurchase = 1,
	SinglePurchaseLimitedUse = 2,
	RecurringCharge = 3,
	RecurringChargeLimitedUse = 4,
	RecurringChargeLimitedUseWithOverages = 5,
	RecurringOption = 6,
	LimitedUseDelayedActivation = 7,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EPaymentMethod {
	None = 0,
	ActivationCode = 1,
	CreditCard = 2,
	Giropay = 3,
	PayPal = 4,
	Ideal = 5,
	PaySafeCard = 6,
	Sofort = 7,
	GuestPass = 8,
	WebMoney = 9,
	MoneyBookers = 10,
	AliPay = 11,
	Yandex = 12,
	Kiosk = 13,
	Qiwi = 14,
	GameStop = 15,
	HardwarePromo = 16,
	MoPay = 17,
	BoletoBancario = 18,
	BoaCompraGold = 19,
	BancoDoBrasilOnline = 20,
	ItauOnline = 21,
	BradescoOnline = 22,
	Pagseguro = 23,
	VisaBrazil = 24,
	AmexBrazil = 25,
	Aura = 26,
	Hipercard = 27,
	MastercardBrazil = 28,
	DinersCardBrazil = 29,
	AuthorizedDevice = 30,
	MOLPoints = 31,
	ClickAndBuy = 32,
	Beeline = 33,
	Konbini = 34,
	EClubPoints = 35,
	CreditCardJapan = 36,
	BankTransferJapan = 37,
	PayEasy = 38,
	Zong = 39,
	CultureVoucher = 40,
	BookVoucher = 41,
	HappymoneyVoucher = 42,
	ConvenientStoreVoucher = 43,
	GameVoucher = 44,
	Multibanco = 45,
	Payshop = 46,
	MaestroBoaCompra = 47,
	OXXO = 48,
	ToditoCash = 49,
	Carnet = 50,
	SPEI = 51,
	ThreePay = 52,
	IsBank = 53,
	Garanti = 54,
	Akbank = 55,
	YapiKredi = 56,
	Halkbank = 57,
	BankAsya = 58,
	Finansbank = 59,
	DenizBank = 60,
	PTT = 61,
	CashU = 62,
	AutoGrant = 64,
	WebMoneyJapan = 65,
	OneCard = 66,
	PSE = 67,
	Exito = 68,
	Efecty = 69,
	Paloto = 70,
	PinValidda = 71,
	MangirKart = 72,
	BancoCreditoDePeru = 73,
	BBVAContinental = 74,
	SafetyPay = 75,
	PagoEfectivo = 76,
	Trustly = 77,
	UnionPay = 78,
	BitCoin = 79,
	Wallet = 128,
	Valve = 129,
	MasterComp = 130,
	Promotional = 131,
	OEMTicket = 256,
	Split = 512,
	Complimentary = 1024,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EPurchaseResultDetail {
	NoDetail = 0,
	AVSFailure = 1,
	InsufficientFunds = 2,
	ContactSupport = 3,
	Timeout = 4,
	InvalidPackage = 5,
	InvalidPaymentMethod = 6,
	InvalidData = 7,
	OthersInProgress = 8,
	AlreadyPurchased = 9,
	WrongPrice = 10,
	FraudCheckFailed = 11,
	CancelledByUser = 12,
	RestrictedCountry = 13,
	BadActivationCode = 14,
	DuplicateActivationCode = 15,
	UseOtherPaymentMethod = 16,
	UseOtherFunctionSource = 17,
	InvalidShippingAddress = 18,
	RegionNotSupported = 19,
	AcctIsBlocked = 20,
	AcctNotVerified = 21,
	InvalidAccount = 22,
	StoreBillingCountryMismatch = 23,
	DoesNotOwnRequiredApp = 24,
	CanceledByNewTransaction = 25,
	ForceCanceledPending = 26,
	FailCurrencyTransProvider = 27,
	FailedCyberCafe = 28,
	NeedsPreApproval = 29,
	PreApprovalDenied = 30,
	WalletCurrencyMismatch = 31,
	EmailNotValidated = 32,
	ExpiredCard = 33,
	TransactionExpired = 34,
	WouldExceedMaxWallet = 35,
	MustLoginPS3AppForPurchase = 36,
	CannotShipToPOBox = 37,
	InsufficientInventory = 38,
	CannotGiftShippedGoods = 39,
	CannotShipInternationally = 40,
	BillingAgreementCancelled = 41,
	InvalidCoupon = 42,
	ExpiredCoupon = 43,
	AccountLocked = 44,
	OtherAbortableInProgress = 45,
	ExceededSteamLimit = 46,
	OverlappingPackagesInCart = 47,
	NoWallet = 48,
	NoCachedPaymentMethod = 49,
	CannotRedeemCodeFromClient = 50,
	PurchaseAmountNoSupportedByProvider = 51,
	OverlappingPackagesInPendingTransaction = 52,
	RateLimited = 53,
	OwnsExcludedApp = 54,
	CreditCardBinMismatchesType = 55,
	CartValueTooHigh = 56,
	BillingAgreementAlreadyExists = 57,
	POSACodeNotActivated = 58,
	CannotShipToCountry = 59,
	HungTransactionCancelled = 60,
	PaypalInternalError = 61,
	UnknownGlobalCollectError = 62,
	InvalidTaxAddress = 63,
	PhysicalProductLimitExceeded = 64,
	PurchaseCannotBeReplayed = 65,
	DelayedCompletion = 66,
	BundleTypeCannotBeGifted = 67,
	BlockedByUSGov = 68,
	ItemsReservedForCommercialUse = 69,
	GiftAlreadyOwned = 70,
	GiftInvalidForRecipientRegion = 71,
	GiftPricingImbalance = 72,
	GiftRecipientNotSpecified = 73,
	ItemsNotAllowedForCommercialUse = 74,
	BusinessStoreCountryCodeMismatch = 75,
	UserAssociatedWithManyCafes = 76,
	UserNotAssociatedWithCafe = 77,
	AddressInvalid = 78,
	CreditCardNumberInvalid = 79,
	CannotShipToMilitaryPostOffice = 80,
	BillingNameInvalidResemblesCreditCard = 81,
	PaymentMethodTemporarilyUnavailable = 82,
	PaymentMethodNotSupportedForProduct = 83,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EIntroducerRouting {
	P2PVoiceChat = 1,
	P2PNetworking = 2,
}

bitflags! {
	#[derive(Serialize, Deserialize)]
	pub struct EServerFlags: i32 {
		const None = 0;
		const Active = 1;
		const Secure = 2;
		const Dedicated = 4;
		const Linux = 8;
		const Passworded = 16;
		const Private = 32;
	}
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EDenyReason {
	InvalidVersion = 1,
	Generic = 2,
	NotLoggedOn = 3,
	NoLicense = 4,
	Cheater = 5,
	LoggedInElseWhere = 6,
	UnknownText = 7,
	IncompatibleAnticheat = 8,
	MemoryCorruption = 9,
	IncompatibleSoftware = 10,
	SteamConnectionLost = 11,
	SteamConnectionError = 12,
	SteamResponseTimedOut = 13,
	SteamValidationStalled = 14,
	SteamOwnerLeftGuestUser = 15,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EClanRank {
	None = 0,
	Owner = 1,
	Officer = 2,
	Member = 3,
	Moderator = 4,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EClanRelationship {
	None = 0,
	Blocked = 1,
	Invited = 2,
	Member = 3,
	Kicked = 4,
	KickAcknowledged = 5,
	PendingApproval = 6,
	RequestDenied = 7,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EAuthSessionResponse {
	OK = 0,
	UserNotConnectedToSteam = 1,
	NoLicenseOrExpired = 2,
	VACBanned = 3,
	LoggedInElseWhere = 4,
	VACCheckTimedOut = 5,
	AuthTicketCanceled = 6,
	AuthTicketInvalidAlreadyUsed = 7,
	AuthTicketInvalid = 8,
	PublisherIssuedBan = 9,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EChatRoomEnterResponse {
	Success = 1,
	DoesntExist = 2,
	NotAllowed = 3,
	Full = 4,
	Error = 5,
	Banned = 6,
	Limited = 7,
	ClanDisabled = 8,
	CommunityBan = 9,
	MemberBlockedYou = 10,
	YouBlockedMember = 11,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EChatRoomType {
	Friend = 1,
	MUC = 2,
	Lobby = 3,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EChatInfoType {
	StateChange = 1,
	InfoUpdate = 2,
	MemberLimitChange = 3,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EChatAction {
	InviteChat = 1,
	Kick = 2,
	Ban = 3,
	UnBan = 4,
	StartVoiceSpeak = 5,
	EndVoiceSpeak = 6,
	LockChat = 7,
	UnlockChat = 8,
	CloseChat = 9,
	SetJoinable = 10,
	SetUnjoinable = 11,
	SetOwner = 12,
	SetInvisibleToFriends = 13,
	SetVisibleToFriends = 14,
	SetModerated = 15,
	SetUnmoderated = 16,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EChatActionResult {
	Success = 1,
	Error = 2,
	NotPermitted = 3,
	NotAllowedOnClanMember = 4,
	NotAllowedOnBannedUser = 5,
	NotAllowedOnChatOwner = 6,
	NotAllowedOnSelf = 7,
	ChatDoesntExist = 8,
	ChatFull = 9,
	VoiceSlotsFull = 10,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EAppInfoSection {
	Unknown = 0,
	All = 1,
	Common = 2,
	Extended = 3,
	Config = 4,
	Stats = 5,
	Install = 6,
	Depots = 7,
	UFS = 10,
	OGG = 11,
	Policies = 13,
	SysReqs = 14,
	Community = 15,
	Store = 16,
	Localization = 17,
	Broadcastgamedata = 18,
	Computed = 19,
	Albummetadata = 20,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EContentDownloadSourceType {
	Invalid = 0,
	CS = 1,
	CDN = 2,
	LCS = 3,
	ProxyCache = 4,
	LANPeer = 5,
	SLS = 6,
	SteamCache = 7,
	OpenCache = 8,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EPlatformType {
	Unknown = 0,
	Win32 = 1,
	Win64 = 2,
	Linux64 = 3,
	OSX = 4,
	PS3 = 5,
	Linux32 = 6,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EOSType {
	Web = -700,
	IOSUnknown = -600,
	IOS1 = -599,
	IOS2 = -598,
	IOS3 = -597,
	IOS4 = -596,
	IOS5 = -595,
	IOS6 = -594,
	IOS6_1 = -593,
	IOS7 = -592,
	IOS7_1 = -591,
	IOS8 = -590,
	IOS8_1 = -589,
	IOS8_2 = -588,
	IOS8_3 = -587,
	IOS8_4 = -586,
	IOS9 = -585,
	IOS9_1 = -584,
	IOS9_2 = -583,
	IOS9_3 = -582,
	IOS10 = -581,
	IOS10_1 = -580,
	IOS10_2 = -579,
	IOS10_3 = -578,
	IOS11 = -577,
	IOS11_1 = -576,
	IOS11_2 = -575,
	IOS11_3 = -574,
	IOS11_4 = -573,
	IOS12 = -572,
	IOS12_1 = -571,
	AndroidUnknown = -500,
	Android6 = -499,
	Android7 = -498,
	Android8 = -497,
	Android9 = -496,
	UMQ = -400,
	PS3 = -300,
	MacOSUnknown = -102,
	MacOS104 = -101,
	MacOS105 = -100,
	MacOS1058 = -99,
	MacOS106 = -95,
	MacOS1063 = -94,
	MacOS1064_slgu = -93,
	MacOS1067 = -92,
	MacOS107 = -90,
	MacOS108 = -89,
	MacOS109 = -88,
	MacOS1010 = -87,
	MacOS1011 = -86,
	MacOS1012 = -85,
	Macos1013 = -84,
	Macos1014 = -83,
	LinuxUnknown = -203,
	Linux22 = -202,
	Linux24 = -201,
	Linux26 = -200,
	Linux32 = -199,
	Linux35 = -198,
	Linux36 = -197,
	Linux310 = -196,
	Linux316 = -195,
	Linux318 = -194,
	Linux3x = -193,
	Linux4x = -192,
	Linux41 = -191,
	Linux44 = -190,
	Linux49 = -189,
	Linux414 = -188,
	Linux419 = -187,
	Linux5x = -186,
	WinUnknown = 0,
	Win311 = 1,
	Win95 = 2,
	Win98 = 3,
	WinME = 4,
	WinNT = 5,
	Win2000 = 6,
	WinXP = 7,
	Win2003 = 8,
	WinVista = 9,
	Windows7 = 10,
	Win2008 = 11,
	Win2012 = 12,
	Windows8 = 13,
	Windows81 = 14,
	Win2012R2 = 15,
	Windows10 = 16,
	Win2016 = 17,
	WinMAX = 18,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EServerType {
	Util = -2,
	Client = -3,
	CServer = -4,
	CEconBase = -5,
	Invalid = -1,
	Shell = 0,
	GM = 1,
	AM = 3,
	BS = 4,
	VS = 5,
	ATS = 6,
	CM = 7,
	FBS = 8,
	BoxMonitor = 9,
	SS = 10,
	DRMS = 11,
	Console = 13,
	PICS = 14,
	ContentStats = 16,
	DP = 17,
	WG = 18,
	SM = 19,
	SLC = 20,
	UFS = 21,
	Community = 24,
	AppInformation = 26,
	Spare = 27,
	FTS = 28,
	SiteLicense = 29,
	PS = 30,
	IS = 31,
	CCS = 32,
	DFS = 33,
	LBS = 34,
	MDS = 35,
	CS = 36,
	GC = 37,
	NS = 38,
	OGS = 39,
	WebAPI = 40,
	UDS = 41,
	MMS = 42,
	GMS = 43,
	KGS = 44,
	UCM = 45,
	RM = 46,
	FS = 47,
	Econ = 48,
	Backpack = 49,
	UGS = 50,
	StoreFeature = 51,
	MoneyStats = 52,
	CRE = 53,
	UMQ = 54,
	Workshop = 55,
	BRP = 56,
	GCH = 57,
	MPAS = 58,
	Trade = 59,
	Secrets = 60,
	Logsink = 61,
	Market = 62,
	Quest = 63,
	WDS = 64,
	ACS = 65,
	PNP = 66,
	TaxForm = 67,
	ExternalMonitor = 68,
	Parental = 69,
	PartnerUpload = 70,
	Partner = 71,
	ES = 72,
	DepotWebContent = 73,
	ExternalConfig = 74,
	GameNotifications = 75,
	MarketRepl = 76,
	MarketSearch = 77,
	Localization = 78,
	Steam2Emulator = 79,
	PublicTest = 80,
	SolrMgr = 81,
	BroadcastRelay = 82,
	BroadcastDirectory = 83,
	VideoManager = 84,
	TradeOffer = 85,
	BroadcastChat = 86,
	Phone = 87,
	AccountScore = 88,
	Support = 89,
	LogRequest = 90,
	LogWorker = 91,
	EmailDelivery = 92,
	InventoryManagement = 93,
	Auth = 94,
	StoreCatalog = 95,
	HLTVRelay = 96,
	IDLS = 97,
	Perf = 98,
	ItemInventory = 99,
	Watchdog = 100,
	AccountHistory = 101,
	Chat = 102,
	Shader = 103,
	AccountHardware = 104,
	WebRTC = 105,
	Giveaway = 106,
	ChatRoom = 107,
	VoiceChat = 108,
	QMS = 109,
	Trust = 110,
	TimeMachine = 111,
	VACDBMaster = 112,
	ContentServerConfig = 113,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EBillingType {
	NoCost = 0,
	BillOnceOnly = 1,
	BillMonthly = 2,
	ProofOfPrepurchaseOnly = 3,
	GuestPass = 4,
	HardwarePromo = 5,
	Gift = 6,
	AutoGrant = 7,
	OEMTicket = 8,
	RecurringOption = 9,
	BillOnceOrCDKey = 10,
	Repurchaseable = 11,
	FreeOnDemand = 12,
	Rental = 13,
	CommercialLicense = 14,
	FreeCommercialLicense = 15,
	NumBillingTypes = 16,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u32)]
pub enum EActivationCodeClass {
	WonCDKey = 0,
	ValveCDKey = 1,
	Doom3CDKey = 2,
	DBLookup = 3,
	Steam2010Key = 4,
	Test = 2147483647,
	Invalid = 4294967295,
}

bitflags! {
	#[derive(Serialize, Deserialize)]
	pub struct EChatMemberStateChange: i32 {
		const Entered = 0x01;
		const Left = 0x02;
		const Disconnected = 0x04;
		const Kicked = 0x08;
		const Banned = 0x10;
		const VoiceSpeaking = 0x1000;
		const VoiceDoneSpeaking = 0x2000;
	}
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ERegionCode {
	USEast = 0x00,
	USWest = 0x01,
	SouthAmerica = 0x02,
	Europe = 0x03,
	Asia = 0x04,
	Australia = 0x05,
	MiddleEast = 0x06,
	Africa = 0x07,
	World = 0xFF,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum ECurrencyCode {
	Invalid = 0,
	USD = 1,
	GBP = 2,
	EUR = 3,
	CHF = 4,
	RUB = 5,
	PLN = 6,
	BRL = 7,
	JPY = 8,
	NOK = 9,
	IDR = 10,
	MYR = 11,
	PHP = 12,
	SGD = 13,
	THB = 14,
	VND = 15,
	KRW = 16,
	TRY = 17,
	UAH = 18,
	MXN = 19,
	CAD = 20,
	AUD = 21,
	NZD = 22,
	CNY = 23,
	INR = 24,
	CLP = 25,
	PEN = 26,
	COP = 27,
	ZAR = 28,
	HKD = 29,
	TWD = 30,
	SAR = 31,
	AED = 32,
	ARS = 34,
	ILS = 35,
	BYN = 36,
	KZT = 37,
	KWD = 38,
	QAR = 39,
	CRC = 40,
	UYU = 41,
}

bitflags! {
	#[derive(Serialize, Deserialize)]
	pub struct EDepotFileFlag: i32 {
		const UserConfig = 1;
		const VersionedUserConfig = 2;
		const Encrypted = 4;
		const ReadOnly = 8;
		const Hidden = 16;
		const Executable = 32;
		const Directory = 64;
		const CustomExecutable = 128;
		const InstallScript = 256;
		const Symlink = 512;
	}
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EWorkshopEnumerationType {
	RankedByVote = 0,
	Recent = 1,
	Trending = 2,
	FavoriteOfFriends = 3,
	VotedByFriends = 4,
	ContentByFriends = 5,
	RecentFromFollowedUsers = 6,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EPublishedFileVisibility {
	Public = 0,
	FriendsOnly = 1,
	Private = 2,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EWorkshopFileType {
	Community	= 0,
	Microtransaction	= 1,
	Collection	= 2,
	Art	= 3,
	Video	= 4,
	Screenshot	= 5,
	Game	= 6,
	Software	= 7,
	Concept	= 8,
	WebGuide	= 9,
	IntegratedGuide	= 10,
	Merch	= 11,
	ControllerBinding	= 12,
	SteamworksAccessInvite = 13,
	SteamVideo = 14,
	GameManagedItem = 15,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EWorkshopFileAction {
	Played = 0,
	Completed = 1,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EEconTradeResponse {
	Accepted = 0,
	Declined = 1,
	TradeBannedInitiator = 2,
	TradeBannedTarget = 3,
	TargetAlreadyTrading = 4,
	Disabled = 5,
	NotLoggedIn = 6,
	Cancel = 7,
	TooSoon = 8,
	TooSoonPenalty = 9,
	ConnectionFailed = 10,
	AlreadyTrading = 11,
	AlreadyHasTradeRequest = 12,
	NoResponse = 13,
	CyberCafeInitiator = 14,
	CyberCafeTarget = 15,
//	SchoolLabInitiator = 16,
	SchoolLabTarget = 16,
	InitiatorBlockedTarget = 18,
	InitiatorNeedsVerifiedEmail = 20,
	InitiatorNeedsSteamGuard = 21,
	TargetAccountCannotTrade = 22,
	InitiatorSteamGuardDuration = 23,
	InitiatorPasswordResetProbation = 24,
	InitiatorNewDeviceCooldown = 25,
	InitiatorSentInvalidCookie = 26,
	NeedsEmailConfirmation = 27,
	InitiatorRecentEmailChange = 28,
	NeedsMobileConfirmation = 29,
	TradingHoldForClearedTradeOffersInitiator = 30,
	WouldExceedMaxAssetCount = 31,
	DisabledInRegion = 32,
	DisabledInPartnerRegion = 33,
	OKToDeliver = 50,
}

bitflags! {
	#[derive(Serialize, Deserialize)]
	pub struct EMarketingMessageFlags: i32 {
		const None = 0;
		const HighPriority = 1;
		const PlatformWindows = 2;
		const PlatformMac = 4;
		const PlatformLinux = 8;
		const PlatformRestrictions  = Self::PlatformWindows.bits | Self::PlatformMac.bits | Self::PlatformLinux.bits;
	}
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum ENewsUpdateType {
	AppNews = 0,
	SteamAds = 1,
	SteamNews = 2,
	CDDBUpdate = 3,
	ClientUpdate = 4,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum ESystemIMType {
	RawText = 0,
	InvalidCard = 1,
	RecurringPurchaseFailed = 2,
	CardWillExpire = 3,
	SubscriptionExpired = 4,
	GuestPassReceived = 5,
	GuestPassGranted = 6,
	GiftRevoked = 7,
	SupportMessage = 8,
	SupportMessageClearAlert = 9,
}

bitflags! {
	#[derive(Serialize, Deserialize)]
	pub struct EChatFlags: i32 {
		const Locked = 1;
		const InvisibleToFriends = 2;
		const Moderated = 4;
		const Unjoinable = 8;
	}
}

bitflags! {
	#[derive(Serialize, Deserialize)]
	pub struct ERemoteStoragePlatform: i32 {
		const None = 0;
		const Windows = 1;
		const OSX = 2;
		const PS3 = 4;
		const Linux = 8;
		const Switch = 16;
		const Android = 32;
		const IPhoneOS = 64;
		const All = -1;
	}
}

bitflags! {
	#[derive(Serialize, Deserialize)]
	pub struct EDRMBlobDownloadType: i32 {
		const Error = 0;
		const File = 1;
		const Parts = 2;
		const Compressed = 4;
		const AllMask = 7;
		const IsJob = 8;
		const HighPriority = 16;
		const AddTimestamp = 32;
		const LowPriority = 64;
	}
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EDRMBlobDownloadErrorDetail {
	None = 0,
	DownloadFailed = 1,
	TargetLocked = 2,
	OpenZip = 3,
	ReadZipDirectory = 4,
	UnexpectedZipEntry = 5,
	UnzipFullFile = 6,
	UnknownBlobType = 7,
	UnzipStrips = 8,
	UnzipMergeGuid = 9,
	UnzipSignature = 10,
	ApplyStrips = 11,
	ApplyMergeGuid = 12,
	ApplySignature = 13,
	AppIdMismatch = 14,
	AppIdUnexpected = 15,
	AppliedSignatureCorrupt = 16,
	ApplyValveSignatureHeader = 17,
	UnzipValveSignatureHeader = 18,
	PathManipulationError = 19,
	TargetLocked_Base = 65536,
	TargetLocked_Max = 131071,
	NextBase = 131072,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EClientStat {
	P2PConnectionsUDP = 0,
	P2PConnectionsRelay = 1,
	P2PGameConnections = 2,
	P2PVoiceConnections = 3,
	BytesDownloaded = 4,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EClientStatAggregateMethod {
	LatestOnly = 0,
	Sum = 1,
	Event = 2,
	Scalar = 3,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum ELeaderboardDataRequest {
	Global = 0,
	GlobalAroundUser = 1,
	Friends = 2,
	Users = 3,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum ELeaderboardSortMethod {
	None = 0,
	Ascending = 1,
	Descending = 2,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum ELeaderboardDisplayType {
	None = 0,
	Numeric = 1,
	TimeSeconds = 2,
	TimeMilliSeconds = 3,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum ELeaderboardUploadScoreMethod {
	None = 0,
	KeepBest = 1,
	ForceUpdate = 2,
}

bitflags! {
	#[derive(Serialize, Deserialize)]
	pub struct EUCMFilePrivacyState: i32 {
		const Invalid = -1;
		const Private = 2;
		const FriendsOnly = 4;
		const Public = 8;
		const All  = Self::Public.bits | Self::FriendsOnly.bits | Self::Private.bits;
	}
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EPublishedFileQueryType {
	RankedByVote = 0,
	RankedByPublicationDate = 1,
	AcceptedForGameRankedByAcceptanceDate = 2,
	RankedByTrend = 3,
	FavoritedByFriendsRankedByPublicationDate = 4,
	CreatedByFriendsRankedByPublicationDate = 5,
	RankedByNumTimesReported = 6,
	CreatedByFollowedUsersRankedByPublicationDate = 7,
	NotYetRated = 8,
	RankedByTotalUniqueSubscriptions = 9,
	RankedByTotalVotesAsc = 10,
	RankedByVotesUp = 11,
	RankedByTextSearch = 12,
	RankedByPlaytimeTrend = 13,
	RankedByTotalPlaytime = 14,
	RankedByAveragePlaytimeTrend = 15,
	RankedByLifetimeAveragePlaytime = 16,
	RankedByPlaytimeSessionsTrend = 17,
	RankedByLifetimePlaytimeSessions = 18,
	RankedByInappropriateContentRating = 19,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EPublishedFileInappropriateProvider {
	Invalid = 0,
	Google = 1,
	Amazon = 2,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EPublishedFileInappropriateResult {
	NotScanned = 0,
	VeryUnlikely = 1,
	Unlikely = 30,
	Possible = 50,
	Likely = 75,
	VeryLikely = 100,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EDisplayStatus {
	Invalid = 0,
	Launching = 1,
	Uninstalling = 2,
	Installing = 3,
	Running = 4,
	Validating = 5,
	Updating = 6,
	Downloading = 7,
	Synchronizing = 8,
	ReadyToInstall = 9,
	ReadyToPreload = 10,
	ReadyToLaunch = 11,
	RegionRestricted = 12,
	PresaleOnly = 13,
	InvalidPlatform = 14,
	ParentalBlocked = 15,
	PreloadOnly = 16,
	BorrowerLocked = 17,
	UpdatePaused = 18,
	UpdateQueued = 19,
	UpdateRequired = 20,
	UpdateDisabled = 21,
	DownloadPaused = 22,
	DownloadQueued = 23,
	DownloadRequired = 24,
	DownloadDisabled = 25,
	LicensePending = 26,
	LicenseExpired = 27,
	AvailForFree = 28,
	AvailToBorrow = 29,
	AvailGuestPass = 30,
	Purchase = 31,
}

bitflags! {
	#[derive(Serialize, Deserialize)]
	pub struct EAppType: i32 {
		const Invalid = 0;
		const Game = 1;
		const Application = 2;
		const Tool = 4;
		const Demo = 8;
		const Deprected = 16;
		const DLC = 32;
		const Guide = 64;
		const Driver = 128;
		const Config = 256;
		const Hardware = 512;
		const Franchise = 1024;
		const Video = 2048;
		const Plugin = 4096;
		const Music = 8192;
		const Series = 16384;
		const Comic = 32768;
		const Beta = 65536;
		const Shortcut = 1073741824;
		const DepotOnly = -2147483648;
	}
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EChatRoomGroupType {
	Default = 0,
	Unmoderated = 1,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EChatroomNotificationLevel {
	Invalid = 0,
	None = 1,
	MentionMe = 2,
	MentionAll = 3,
	AllMessages = 4,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EChatRoomMemberStateChange {
	Invalid = 0,
	Joined = 1,
	Parted = 2,
	Kicked = 3,
	Invited = 4,
	RankChanged = 7,
	InviteDismissed = 8,
	Muted = 9,
	Banned = 10,
	RolesChanged = 12,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EChatRoomServerMsg {
	Invalid = 0,
	RenameChatRoom = 1,
	Joined = 2,
	Parted = 3,
	Kicked = 4,
	Invited = 5,
	InviteDismissed = 8,
	ChatRoomTaglineChanged = 9,
	ChatRoomAvatarChanged = 10,
	AppCustom = 11,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EChatRoomGroupRank {
	Default = 0,
	Viewer = 10,
	Guest = 15,
	Member = 20,
	Moderator = 30,
	Officer = 40,
	Owner = 50,
}

bitflags! {
	#[derive(Serialize, Deserialize)]
	pub struct EChatRoomGroupPermissions: i32 {
		const Default = 0;
		const Valid = 1;
		const CanInvite = 2;
		const CanKick = 4;
		const CanBan = 8;
		const CanAdminChannel = 16;
	}
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EChatRoomGroupAction {
	Default = 0,
	CreateRenameDeleteChannel = 1,
	Kick = 2,
	Ban = 3,
	Invite = 4,
	ChangeTaglineAvatarName = 5,
	Chat = 6,
	ViewHistory = 7,
	ChangeGroupRoles = 8,
	ChangeUserRoles = 9,
	MentionAll = 10,
	SetWatchingBroadcast = 11,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EChatRoomJoinState {
	Default = 0,
	None = 1,
	Joined = 2,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum EVoiceCallState {
	None = 0,
	ScheduledInitiate = 1,
	RequestedMicAccess = 2,
	LocalMicOnly = 3,
	CreatePeerConnection = 4,
	InitatedWebRTCSession = 5,
	WebRTCConnectedWaitingOnIceConnected = 6,
	RequestedPermission = 7,
	NotifyingVoiceChatOfWebRTCSession = 8,
	Connected = 9,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize_repr, Deserialize_repr)]
#[repr(i32)]
pub enum ETradeOfferState {
	Invalid = 1,
	Active = 2,
	Accepted = 3,
	Countered = 4,
	Expired = 5,
	Canceled = 6,
	Declined = 7,
	InvalidItems = 8,
	CreatedNeedsConfirmation = 9,
	CanceledBySecondFactor = 10,
	InEscrow = 11,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize_repr, Deserialize_repr)]
#[repr(i32)]
pub enum ETradeOfferConfirmationMethod {
	Invalid = 0,
	Email = 1,
	MobileApp = 2,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum ELobbyType {
	    Private = 0,
	    FriendsOnly = 1,
	    Public = 2,
	    Invisible = 3,
	    PrivateUnique = 4,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum ELobbyFilterType {
	    String = 0,
	    Numerical = 1,
	    SlotsAvailable = 2,
	    NearValue = 3,
	    Distance = 4,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum ELobbyComparison {
	    EqualToOrLessThan = -2,
	    LessThan = -1,
	    Equal = 0,
	    GreaterThan = 1,
	    EqualToOrGreaterThan = 2,
	    NotEqual = 3,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum ELobbyDistanceFilter {
	    Close = 0,
	    Default = 1,
	    Far = 2,
	    Worldwide = 3,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum ESteamIPv6ConnectivityProtocol {
	Invalid = 0,
	Http = 1,
	Udp = 2,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum ESteamIPv6ConnectivityState {
	Unknown = 0,
	Good = 1,
	Bad = 2,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum ESteamRealm {
	Unknown = 0,
	SteamGlobal = 1,
	SteamChina = 2,
}

#[derive(FromPrimitive, ToPrimitive, Clone, Debug, PartialEq, Eq, Serialize_repr, Deserialize_repr)]
#[repr(i32)]
pub enum EResult {
	Invalid = 0,
	OK = 1,
	Fail = 2,
	NoConnection = 3,
	InvalidPassword = 5,
	LoggedInElsewhere = 6,
	InvalidProtocolVer = 7,
	InvalidParam = 8,
	FileNotFound = 9,
	Busy = 10,
	InvalidState = 11,
	InvalidName = 12,
	InvalidEmail = 13,
	DuplicateName = 14,
	AccessDenied = 15,
	Timeout = 16,
	Banned = 17,
	AccountNotFound = 18,
	InvalidSteamID = 19,
	ServiceUnavailable = 20,
	NotLoggedOn = 21,
	Pending = 22,
	EncryptionFailure = 23,
	InsufficientPrivilege = 24,
	LimitExceeded = 25,
	Revoked = 26,
	Expired = 27,
	AlreadyRedeemed = 28,
	DuplicateRequest = 29,
	AlreadyOwned = 30,
	IPNotFound = 31,
	PersistFailed = 32,
	LockingFailed = 33,
	LogonSessionReplaced = 34,
	ConnectFailed = 35,
	HandshakeFailed = 36,
	IOFailure = 37,
	RemoteDisconnect = 38,
	ShoppingCartNotFound = 39,
	Blocked = 40,
	Ignored = 41,
	NoMatch = 42,
	AccountDisabled = 43,
	ServiceReadOnly = 44,
	AccountNotFeatured = 45,
	AdministratorOK = 46,
	ContentVersion = 47,
	TryAnotherCM = 48,
	PasswordRequiredToKickSession = 49,
	AlreadyLoggedInElsewhere = 50,
	Suspended = 51,
	Cancelled = 52,
	DataCorruption = 53,
	DiskFull = 54,
	RemoteCallFailed = 55,
	PasswordUnset = 56,
	ExternalAccountUnlinked = 57,
	PSNTicketInvalid = 58,
	ExternalAccountAlreadyLinked = 59,
	RemoteFileConflict = 60,
	IllegalPassword = 61,
	SameAsPreviousValue = 62,
	AccountLogonDenied = 63,
	CannotUseOldPassword = 64,
	InvalidLoginAuthCode = 65,
	AccountLogonDeniedNoMail = 66,
	HardwareNotCapableOfIPT = 67,
	IPTInitError = 68,
	ParentalControlRestricted = 69,
	FacebookQueryError = 70,
	ExpiredLoginAuthCode = 71,
	IPLoginRestrictionFailed = 72,
	AccountLockedDown = 73,
	AccountLogonDeniedVerifiedEmailRequired = 74,
	NoMatchingURL = 75,
	BadResponse = 76,
	RequirePasswordReEntry = 77,
	ValueOutOfRange = 78,
	UnexpectedError = 79,
	Disabled = 80,
	InvalidCEGSubmission = 81,
	RestrictedDevice = 82,
	RegionLocked = 83,
	RateLimitExceeded = 84,
	AccountLoginDeniedNeedTwoFactor = 85,
	ItemDeleted = 86,
	AccountLoginDeniedThrottle = 87,
	TwoFactorCodeMismatch = 88,
	TwoFactorActivationCodeMismatch = 89,
	AccountAssociatedToMultiplePartners = 90,
	NotModified = 91,
	NoMobileDevice = 92,
	TimeNotSynced = 93,
	SMSCodeFailed = 94,
	AccountLimitExceeded = 95,
	AccountActivityLimitExceeded = 96,
	PhoneActivityLimitExceeded = 97,
	RefundToWallet = 98,
	EmailSendFailure = 99,
	NotSettled = 100,
	NeedCaptcha = 101,
	GSLTDenied = 102,
	GSOwnerDenied = 103,
	InvalidItemType = 104,
	IPBanned = 105,
	GSLTExpired = 106,
	InsufficientFunds = 107,
	TooManyPending = 108,
	NoSiteLicensesFound = 109,
	WGNetworkSendExceeded = 110,
	AccountNotFriends = 111,
	LimitedUserAccount = 112,
	CantRemoveItem = 113,
}

#[derive(FromPrimitive, ToPrimitive, Hash, Clone, Debug, PartialEq, Eq, Serialize_repr, Deserialize_repr)]
#[repr(i32)]
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

