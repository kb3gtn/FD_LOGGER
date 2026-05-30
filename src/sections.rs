use serde::Serialize;

pub const VALID_ABBREVS: &[&str] = &[
    // District 1
    "CT", "EMA", "ME", "NH", "RI", "VT", "WMA",
    // District 2
    "ENY", "NLI", "NNJ", "NNY", "SNJ", "WNY",
    // District 3
    "DE", "EPA", "MDC", "WPA",
    // District 4
    "AL", "GA", "KY", "NC", "NFL", "PR", "SC", "SFL", "TN", "VA", "VI", "WCF",
    // District 5
    "AR", "LA", "MS", "NM", "NTX", "OK", "STX", "WTX",
    // District 6
    "EB", "LAX", "ORG", "PAC", "SB", "SCV", "SDG", "SF", "SJV", "SV",
    // District 7
    "AK", "AZ", "EWA", "ID", "MT", "NV", "OR", "UT", "WWA", "WY",
    // District 8
    "MI", "OH", "WV",
    // District 9
    "IL", "IN", "WI",
    // District 0
    "CO", "IA", "KS", "MN", "MO", "ND", "NE", "SD",
    // Canada
    "AB", "BC", "GTA", "MAR", "MB", "NL", "NT", "ONE", "ONN", "ONS", "PE", "QC", "SK",
    // Non-US/Canada
    "DX",
];

#[derive(Serialize, Clone)]
pub struct SectionEntry {
    pub abbrev: String,
    pub name: String,
}

#[derive(Serialize, Clone)]
pub struct District {
    pub label: String,
    pub sections: Vec<SectionEntry>,
}

macro_rules! s {
    ($a:expr, $n:expr) => {
        SectionEntry { abbrev: $a.to_string(), name: $n.to_string() }
    };
}

macro_rules! d {
    ($l:expr; $($a:expr, $n:expr);*) => {
        District {
            label: $l.to_string(),
            sections: vec![$( s!($a, $n) ),*],
        }
    };
}

pub fn all_districts() -> Vec<District> {
    vec![
        d!("1 – New England";
            "CT",  "Connecticut";
            "EMA", "Eastern Massachusetts";
            "ME",  "Maine";
            "NH",  "New Hampshire";
            "RI",  "Rhode Island";
            "VT",  "Vermont";
            "WMA", "Western Massachusetts"
        ),
        d!("2 – New York / New Jersey";
            "ENY", "Eastern New York";
            "NLI", "NYC / Long Island";
            "NNJ", "Northern New Jersey";
            "NNY", "Northern New York";
            "SNJ", "Southern New Jersey";
            "WNY", "Western New York"
        ),
        d!("3 – Mid-Atlantic";
            "DE",  "Delaware";
            "EPA", "Eastern Pennsylvania";
            "MDC", "Maryland – DC";
            "WPA", "Western Pennsylvania"
        ),
        d!("4 – Southeast";
            "AL",  "Alabama";
            "GA",  "Georgia";
            "KY",  "Kentucky";
            "NC",  "North Carolina";
            "NFL", "Northern Florida";
            "PR",  "Puerto Rico";
            "SC",  "South Carolina";
            "SFL", "Southern Florida";
            "TN",  "Tennessee";
            "VA",  "Virginia";
            "VI",  "US Virgin Islands";
            "WCF", "West Central Florida"
        ),
        d!("5 – South Central";
            "AR",  "Arkansas";
            "LA",  "Louisiana";
            "MS",  "Mississippi";
            "NM",  "New Mexico";
            "NTX", "North Texas";
            "OK",  "Oklahoma";
            "STX", "South Texas";
            "WTX", "West Texas"
        ),
        d!("6 – California";
            "EB",  "East Bay";
            "LAX", "Los Angeles";
            "ORG", "Orange";
            "PAC", "Pacific";
            "SB",  "Santa Barbara";
            "SCV", "Santa Clara Valley";
            "SDG", "San Diego";
            "SF",  "San Francisco";
            "SJV", "San Joaquin Valley";
            "SV",  "Sacramento Valley"
        ),
        d!("7 – Northwest";
            "AK",  "Alaska";
            "AZ",  "Arizona";
            "EWA", "Eastern Washington";
            "ID",  "Idaho";
            "MT",  "Montana";
            "NV",  "Nevada";
            "OR",  "Oregon";
            "UT",  "Utah";
            "WWA", "Western Washington";
            "WY",  "Wyoming"
        ),
        d!("8 – Great Lakes";
            "MI",  "Michigan";
            "OH",  "Ohio";
            "WV",  "West Virginia"
        ),
        d!("9 – Midwest";
            "IL",  "Illinois";
            "IN",  "Indiana";
            "WI",  "Wisconsin"
        ),
        d!("0 – Plains";
            "CO",  "Colorado";
            "IA",  "Iowa";
            "KS",  "Kansas";
            "MN",  "Minnesota";
            "MO",  "Missouri";
            "ND",  "North Dakota";
            "NE",  "Nebraska";
            "SD",  "South Dakota"
        ),
        d!("Canada";
            "AB",  "Alberta";
            "BC",  "British Columbia";
            "GTA", "Greater Toronto Area";
            "MAR", "Maritime";
            "MB",  "Manitoba";
            "NL",  "Newfoundland/Labrador";
            "NT",  "Northern Territories";
            "ONE", "Ontario East";
            "ONN", "Ontario North";
            "ONS", "Ontario South";
            "PE",  "Prince Edward Island";
            "QC",  "Quebec";
            "SK",  "Saskatchewan"
        ),
        d!("DX";
            "DX",  "Non-US or Canadian station"
        ),
    ]
}
