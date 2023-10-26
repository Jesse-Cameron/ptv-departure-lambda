use phf::phf_map;

pub static STATIONS_MAP: phf::Map<&'static str, u32> = phf_map! {
    "bell" => 1019,
    "clifton_hill" => 1041,
    "collingwood" => 1043,
    "croxton" => 1047,
    "epping" => 1063,
    "flagstaff" => 1068,
    "flinders_street" => 1071,
    "hawkstowe" => 1227,
    "jolimont-mcg" => 1104,
    "keon_park" => 1109,
    "lalor" => 1112,
    "melbourne_central" => 1120,
    "mernda" => 1228,
    "merri" => 1125,
    "middle_gorge" => 1226,
    "north_richmond" => 1145,
    "northcote" => 1147,
    "parliament" => 1155,
    "preston" => 1159,
    "regent" => 1160,
    "reservoir" => 1161,
    "rushall" => 1170,
    "ruthven" => 1171,
    "south_morang" => 1224,
    "southern cross" => 1181,
    "thomastown" => 1192,
    "thornbury" => 1193,
    "victoria_park" => 1201,
    "west_richmond" => 1207,
};

pub fn get_stop_id_from_name(station: &str) -> Option<u32> {
    STATIONS_MAP.get(station).copied()
}
