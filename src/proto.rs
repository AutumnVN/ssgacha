use prost::Message;

#[derive(Clone, PartialEq, Message)]
pub struct IkeReq {
    #[prost(int32, optional, tag = "1")]
    pub proto_version: Option<i32>,
    #[prost(int64, optional, tag = "2")]
    pub client_ts: Option<i64>,
    #[prost(bytes = "vec", optional, tag = "3")]
    pub pub_key: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, Message)]
pub struct IkeResp {
    #[prost(string, optional, tag = "1")]
    pub token: Option<String>,
    #[prost(int32, optional, tag = "2")]
    pub cipher: Option<i32>,
    #[prost(int64, optional, tag = "3")]
    pub server_ts: Option<i64>,
    #[prost(bytes = "vec", optional, tag = "4")]
    pub pub_key: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, Message)]
pub struct OfficialOverseas {
    #[prost(string, optional, tag = "1")]
    pub uid: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub token: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct Regular {
    #[prost(string, optional, tag = "1")]
    pub name: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub pass: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct Official {
    #[prost(uint64, optional, tag = "1")]
    pub uid: Option<u64>,
    #[prost(string, optional, tag = "2")]
    pub token: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct LoginReq {
    #[prost(message, optional, tag = "1")]
    pub account: Option<Regular>,
    #[prost(message, optional, tag = "2")]
    pub official: Option<Official>,
    #[prost(message, optional, tag = "3")]
    pub official_overseas: Option<OfficialOverseas>,
    #[prost(string, optional, tag = "4")]
    pub token: Option<String>,
    #[prost(int32, optional, tag = "11")]
    pub platform: Option<i32>,
    #[prost(string, optional, tag = "12")]
    pub language: Option<String>,
    #[prost(string, optional, tag = "13")]
    pub channel: Option<String>,
    #[prost(string, optional, tag = "14")]
    pub device: Option<String>,
    #[prost(string, optional, tag = "15")]
    pub version: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct LoginResp {
    #[prost(string, optional, tag = "1")]
    pub token: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct ErrorMsg {
    #[prost(uint32, optional, tag = "1")]
    pub code: Option<u32>,
    #[prost(string, repeated, tag = "2")]
    pub arguments: Vec<String>,
    #[prost(uint64, optional, tag = "3")]
    pub trace_id: Option<u64>,
    #[prost(int32, optional, tag = "4")]
    pub action: Option<i32>,
}

#[derive(Clone, PartialEq, Message)]
pub struct Ui32 {
    #[prost(uint32, optional, tag = "1")]
    pub value: Option<u32>,
}

#[derive(Clone, PartialEq, Message)]
pub struct GachaHistory {
    #[prost(uint32, repeated, packed = "false", tag = "1")]
    pub ids: Vec<u32>,
    #[prost(int64, optional, tag = "2")]
    pub time: Option<i64>,
    #[prost(uint32, optional, tag = "3")]
    pub gid: Option<u32>,
}

#[derive(Clone, PartialEq, Message)]
pub struct GachaHistories {
    #[prost(message, repeated, tag = "1")]
    pub list: Vec<GachaHistory>,
}

#[derive(Clone, PartialEq, Message)]
pub struct GachaInfo {
    #[prost(uint32, optional, tag = "1")]
    pub id: Option<u32>,
    #[prost(uint32, optional, tag = "2")]
    pub days_count: Option<u32>,
    #[prost(uint32, optional, tag = "3")]
    pub aup_miss_times: Option<u32>,
    #[prost(uint32, optional, tag = "4")]
    pub a_miss_times: Option<u32>,
    #[prost(bytes = "vec", optional, tag = "10")]
    pub rewards_records: Option<Vec<u8>>,
    #[prost(bool, optional, tag = "11")]
    pub reve_first_ten_reward: Option<bool>,
    #[prost(bool, optional, tag = "12")]
    pub recv_guarantee_reward: Option<bool>,
    #[prost(uint32, optional, tag = "13")]
    pub gacha_total_times: Option<u32>,
    #[prost(uint32, optional, tag = "14")]
    pub total_times: Option<u32>,
    #[prost(uint32, optional, tag = "15")]
    pub aup_guarantee_times: Option<u32>,
}

#[derive(Clone, PartialEq, Message)]
pub struct GachaInformationResp {
    #[prost(message, repeated, tag = "1")]
    pub information: Vec<GachaInfo>,
}

#[derive(Clone, PartialEq, Message)]
pub struct AccInfo {
    #[prost(uint64, optional, tag = "1")]
    pub id: Option<u64>,
    #[prost(string, optional, tag = "2")]
    pub nickname: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct PlayerInfo {
    #[prost(message, optional, tag = "1")]
    pub acc: Option<AccInfo>,
}
