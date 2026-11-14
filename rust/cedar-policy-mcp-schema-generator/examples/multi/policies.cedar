permit (
    principal,
    action in Main::Weather::Action::"AllActions",
    resource in Main::Gateway::"gw1"
) when {
    context.system.region == "us-east-1" &&
    context.input has city && context.input.city == "Paris"
};

permit (
    principal is Main::OAuthUser,
    action in Main::Billing::Action::"ReadOnly",
    resource == Main::Billing::Target::"billing"
) when { context.input.account_id == "acct2" };
