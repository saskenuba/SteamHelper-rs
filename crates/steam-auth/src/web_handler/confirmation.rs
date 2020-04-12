struct Confirmation {}

// 1. generate confirmation hash
// 2. get confirmations

enum ConfirmationType {
    Unknown,
    Generic,
    Trade,
    Market,

    // We're missing information about definition of number 4 type
    PhoneNumberChange = 5,
    AccountRecovery = 6,
}

fn get_web_confirmation_document() {}


enum InventoryPrivacy {
    Unknown,
    Private,
    FriendsOnly,
    Public,
}