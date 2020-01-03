# StovBot - Discord and Twitch bot

table command {
}

table variable {
    name text
    value text
    created date
    updated date
}

table user_variable {
    name text
    user text
    value text
    created date
    updated date
}

table array_variable {
    index int
    name text
    value text
    created date
    updated date
}

table user {
    twitch_user text
    discord_user text
}
