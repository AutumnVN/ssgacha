const crypto = require('crypto');
const childProcess = require('child_process');
const fs = require('fs');
const os = require('os');
const path = require('path');
const readline = require('readline');
const protobuf = require('protobufjs');

const REGION_ORDER = ['en', 'jp', 'kr', 'tw', 'cn'];

const CLIENT_VERSION = '727.727.727.7272727';
const SDK_VERSION = '4.16.0';
const SDK_VERSION_CN = '1.20.0';
const DEVICE_MODEL = 'AutumnVN';
const SPARK_THRESHOLD = 120;

const REGIONS = {
    en: {
        key: 'en',
        label: 'EN',
        sdkUrl: 'https://en-sdk-api.yostarplat.com',
        serverUrl: 'https://nova.stellasora.global',
        garbleKey: Buffer.from('xNdVF^XTa6T3HCUATMQ@sKMLzAw&%L!3', 'ascii'),
        language: 'en_US',
        authLang: 'en',
        pid: 'US-NOVA',
        dataPath: 'EN',
        sdkVersion: SDK_VERSION,
        loginField: 'OfficialOverseas',
    },
    jp: {
        key: 'jp',
        label: 'JP',
        sdkUrl: 'https://jp-sdk-api.yostarplat.com',
        serverUrl: 'https://nova.stellasora.jp',
        garbleKey: Buffer.from('yX5Gt64PVvVH6$qwBXaPJC*LZKoK5mYh', 'ascii'),
        language: 'ja_JP',
        authLang: 'ja',
        pid: 'JP-NOVA',
        dataPath: 'JP',
        sdkVersion: SDK_VERSION,
        loginField: 'OfficialOverseas',
    },
    kr: {
        key: 'kr',
        label: 'KR',
        sdkUrl: 'https://jp-sdk-api.yostarplat.com',
        serverUrl: 'https://nova.stellasora.kr',
        garbleKey: Buffer.from('25hdume9H#*6hHn@d9hSF7tekTwN#JYj', 'ascii'),
        language: 'ko_KR',
        authLang: 'ko',
        pid: 'KR-NOVA',
        dataPath: 'KR',
        sdkVersion: SDK_VERSION,
        loginField: 'OfficialOverseas',
    },
    tw: {
        key: 'tw',
        label: 'TW',
        sdkUrl: 'https://jp-sdk-api.yostarplat.com',
        serverUrl: 'https://nova.stargazer-games.com',
        garbleKey: Buffer.from('N&mfco452ZH5!nE3s&o5uxB57UGPENVo', 'ascii'),
        language: 'zh_TW',
        authLang: 'zh-Hant',
        pid: 'TW-NOVA',
        dataPath: 'TW',
        sdkVersion: SDK_VERSION,
        loginField: 'OfficialOverseas',
    },
    cn: {
        key: 'cn',
        label: 'CN',
        sdkUrl: 'https://sdk-api.yostar.cn',
        serverUrl: 'https://nova.yostar.cn',
        garbleKey: Buffer.from('QW*Wi7fKjLk!T82Qf2nEGZA%nSC!D9qV', 'ascii'),
        language: 'zh_CN',
        authLang: 'ChineseSimplified',
        pid: 'CN-NOVA',
        dataPath: 'CN',
        sdkVersion: SDK_VERSION_CN,
        loginField: 'Official',
        authMode: 'cn',
    },
};

const DATA_BASE = 'https://raw.githubusercontent.com/AutumnVN/ss-data/refs/heads/main';
const DATA_URLS = {
    banners: (region) => `${DATA_BASE}/${region.dataPath}/bin/Gacha.json`,
    storages: (region) => `${DATA_BASE}/${region.dataPath}/bin/GachaStorage.json`,
    bannerLang: (region) => `${DATA_BASE}/${region.dataPath}/language/${region.language}/Gacha.json`,
    storageLang: (region) => `${DATA_BASE}/${region.dataPath}/language/${region.language}/GachaStorage.json`,
    gacha: `${DATA_BASE}/gacha.json`,
    items: `${DATA_BASE}/item.json`,
};

const protoText = `
syntax = "proto2";
package proto;

message IKEReq {
    optional int32 ProtoVersion = 1;
    optional int64 ClientTs = 2;
    optional bytes PubKey = 3;
}

message IKEResp {
    optional string Token = 1;
    optional int32 Cipher = 2;
    optional int64 ServerTs = 3;
    optional bytes PubKey = 4;
}

message OfficialOverseas {
    optional string Uid = 1;
    optional string Token = 2;
}

message Regular {
    optional string Name = 1;
    optional string Pass = 2;
}

message Official {
    optional uint64 Uid = 1;
    optional string Token = 2;
}

message LoginReq {
    optional Regular Account = 1;
    optional Official Official = 2;
    optional OfficialOverseas OfficialOverseas = 3;
    optional string Token = 4;
    optional int32 Platform = 11;
    optional string Language = 12;
    optional string Channel = 13;
    optional string Device = 14;
    optional string Version = 15;
}

message LoginResp {
    optional string Token = 1;
}

message Error {
    optional uint32 Code = 1;
    repeated string Arguments = 2;
    optional uint64 TraceId = 3;
    optional int32 Action = 4;
}

message UI32 {
    optional uint32 Value = 1;
}

message GachaHistory {
    repeated uint32 Ids = 1;
    optional int64 Time = 2;
    optional uint32 Gid = 3;
}

message GachaHistories {
    repeated GachaHistory List = 1;
}

message GachaInfo {
    optional uint32 Id = 1;
    optional uint32 DaysCount = 2;
    optional uint32 AupMissTimes = 3;
    optional uint32 AMissTimes = 4;
    optional bytes RewardsRecords = 10;
    optional bool ReveFirstTenReward = 11;
    optional bool RecvGuaranteeReward = 12;
    optional uint32 GachaTotalTimes = 13;
    optional uint32 TotalTimes = 14;
    optional uint32 AupGuaranteeTimes = 15;
}

message GachaInformationResp {
    repeated GachaInfo Information = 1;
}

message AccInfo {
    optional uint64 Id = 1;
    optional string NickName = 2;
}

message PlayerInfo {
    optional AccInfo Acc = 1;
}
`;

const root = protobuf.parse(protoText).root;
const IKEReq = root.lookupType('proto.IKEReq');
const IKEResp = root.lookupType('proto.IKEResp');
const LoginReq = root.lookupType('proto.LoginReq');
const LoginResp = root.lookupType('proto.LoginResp');
const ErrorType = root.lookupType('proto.Error');
const UI32 = root.lookupType('proto.UI32');
const GachaHistories = root.lookupType('proto.GachaHistories');
const GachaInformationResp = root.lookupType('proto.GachaInformationResp');
const PlayerInfo = root.lookupType('proto.PlayerInfo');

const MSG = {
    loginReq: 4,
    loginSucceedAck: 5,
    loginFailedAck: 6,
    playerDataReq: 1001,
    playerDataSucceedAck: 1002,
    gachaInformationReq: 6004,
    gachaInformationSucceedAck: 6005,
    gachaHistoriesReq: 6007,
    gachaHistoriesSucceedAck: 6008,
    systemFailedAck: 10000,
};

const REGION_ALIASES = {
    en: 'en',
    us: 'en',
    na: 'en',
    eu: 'en',
    jp: 'jp',
    ja: 'jp',
    kr: 'kr',
    ko: 'kr',
    tw: 'tw',
    'zh-tw': 'tw',
    'zh_tw': 'tw',
    cn: 'cn',
    'zh-cn': 'cn',
    'zh_cn': 'cn',
    zh: 'cn',
};

function stellaSoraDirectories() {
    const localLow = path.join(os.homedir(), 'AppData', 'LocalLow');
    const dirs = [];
    let entries;
    try {
        entries = fs.readdirSync(localLow, { withFileTypes: true });
    } catch (err) {
        if (err.code !== 'ENOENT') {
            console.error(`Could not read ${localLow}: ${err.message}`);
        }
        return [];
    }

    for (const entry of entries) {
        const stella = path.join(localLow, entry.name, 'StellaSora');
        try {
            if (fs.statSync(stella).isDirectory()) dirs.push(stella);
        } catch (err) {
            if (err.code !== 'ENOENT') {
                console.error(`Could not access ${stella}: ${err.message}`);
            }
        }
    }

    return [...new Set(dirs)];
}

function findYoSdkFiles() {
    const files = [];
    for (const dir of stellaSoraDirectories()) {
        let entries;
        try {
            entries = fs.readdirSync(dir, { withFileTypes: true });
        } catch (err) {
            console.error(`Could not read ${dir}: ${err.message}`);
            continue;
        }
        let found = false;
        for (const entry of entries) {
            if (entry.isFile() && /^YoSDK_UserInfo/i.test(entry.name)) {
                files.push(path.join(dir, entry.name));
                found = true;
            }
        }
        if (!found) {
            console.error(`No YoSDK_UserInfo* files in ${dir}`);
        }
    }

    const localLow = path.join(os.homedir(), 'AppData', 'LocalLow');
    let companies;
    try {
        companies = fs.readdirSync(localLow, { withFileTypes: true });
    } catch (_) {
        return files;
    }
    for (const company of companies) {
        const companyPath = path.join(localLow, company.name);
        let companyStat;
        try {
            companyStat = fs.statSync(companyPath);
        } catch (_) {
            continue;
        }
        if (!companyStat.isDirectory()) continue;

        const cn = path.join(
            companyPath,
            'xtlr',
            'CN_SDK_Cache',
            'official',
            'sdk_cn_official_userinfo',
        );
        try {
            if (fs.statSync(cn).isFile()) files.push(cn);
        } catch (_) {
        }
    }

    return files;
}

function isCredentialFile(file) {
    const name = path.basename(file).toLowerCase();
    return name.startsWith('yosdk_userinfo') || name === 'sdk_cn_official_userinfo';
}

function readJsonFile(file) {
    try {
        const text = fs.readFileSync(file, 'utf8').replace(/^\uFEFF/, '').trim();
        return JSON.parse(text);
    } catch (_) {
        return null;
    }
}

function parseJsonValue(value) {
    if (value && typeof value === 'object') return value;
    if (typeof value !== 'string' || !value.trim()) return null;
    try {
        return JSON.parse(value);
    } catch (_) {
        return null;
    }
}

function normalizeRegion(value) {
    if (!value) return null;
    const lower = String(value).trim().toLowerCase().replace(/_/g, '-');
    return REGION_ALIASES[lower];
}

function yoSdkRegionFromFile(file) {
    const name = path.basename(file);
    if (name.toLowerCase() === 'sdk_cn_official_userinfo') return 'cn';
    const match = /^YoSDK_UserInfo(?:[_-](.+))?$/i.exec(name);
    if (!match || !match[1]) return null;
    return normalizeRegion(match[1]);
}

function stringValue(value) {
    if (value === undefined || value === null) return null;
    const text = String(value).trim();
    return text || null;
}

function compareUids(left, right) {
    const leftNumber = Number(left);
    const rightNumber = Number(right);
    if (Number.isFinite(leftNumber) && Number.isFinite(rightNumber)) {
        return leftNumber - rightNumber;
    }
    return String(left).localeCompare(String(right), undefined, { numeric: true });
}

const ENV_FILE = path.join(process.cwd(), '.env');
let _unityDeviceId;

function parseEnvFile(text) {
    const values = {};
    for (const rawLine of text.replace(/^\uFEFF/, '').split(/\r?\n/)) {
        const line = rawLine.trim();
        if (!line || line.startsWith('#')) continue;
        const equals = line.indexOf('=');
        if (equals < 0) continue;
        const key = line.slice(0, equals).trim();
        if (!key) continue;
        let value = line.slice(equals + 1).trim();
        if ((value.startsWith('"') && value.endsWith('"'))
            || (value.startsWith("'") && value.endsWith("'"))) {
            value = value.slice(1, -1);
        }
        values[key] = value;
    }
    return values;
}

function readEnvFile() {
    try {
        return parseEnvFile(fs.readFileSync(ENV_FILE, 'utf8'));
    } catch (_) {
        return {};
    }
}

function envValue(key) {
    const fromFile = readEnvFile()[key];
    if (fromFile && fromFile.trim()) return fromFile.trim();
    const fromProcess = process.env[key];
    return fromProcess && fromProcess.trim() ? fromProcess.trim() : null;
}

function loadSavedLogin(regionKey) {
    const suffix = regionKey.toUpperCase();
    const token = envValue(`TOKEN_${suffix}`);
    if (!token) return null;
    return {
        uid: envValue(`UID_${suffix}`) || '',
        token,
        device: envValue('DEVICE') || '',
    };
}

function saveLogin(regionKey, uid, token, device) {
    try {
        let lines = [];
        try {
            lines = fs.readFileSync(ENV_FILE, 'utf8').replace(/^\uFEFF/, '').split(/\r?\n/);
        } catch (_) {
        }
        while (lines.length > 0 && lines[lines.length - 1] === '') lines.pop();

        const suffix = regionKey.toUpperCase();
        const tokenKey = `TOKEN_${suffix}`;
        const uidKey = `UID_${suffix}`;
        const hasDevice = lines.some((line) => {
            const trimmed = line.trim();
            if (!trimmed.startsWith('DEVICE=')) return false;
            const value = trimmed.slice('DEVICE='.length).trim().replace(/^['"]|['"]$/g, '');
            return Boolean(value);
        });

        lines = lines.filter((line) => {
            const trimmed = line.trim();
            return !trimmed.startsWith(`${tokenKey}=`) && !trimmed.startsWith(`${uidKey}=`);
        });
        if (!hasDevice) {
            lines = lines.filter((line) => !line.trim().startsWith('DEVICE='));
            lines.push(`DEVICE=${device}`);
        }
        lines.push(`${tokenKey}=${token}`);
        lines.push(`${uidKey}=${uid}`);
        fs.writeFileSync(ENV_FILE, `${lines.join('\n')}\n`, 'utf8');
    } catch (err) {
        console.error(`Could not save login to .env: ${err.message}`);
    }
}

function runCommand(command, args) {
    try {
        const output = childProcess.execFileSync(command, args, {
            encoding: 'utf8',
            timeout: 10000,
            windowsHide: true,
        });
        return output.trim() || null;
    } catch (_) {
        return null;
    }
}

function queryPowerShell(className, field) {
    return runCommand('powershell', [
        '-NoProfile',
        '-NonInteractive',
        '-Command',
        `(Get-CimInstance ${className}).${field}`,
    ]);
}

function queryWmic(className, field) {
    const output = runCommand('wmic', [className, 'get', field, '/value']);
    if (!output) return null;
    const prefix = `${field}=`;
    for (const line of output.split(/\r?\n/)) {
        const trimmed = line.trim();
        if (!trimmed.startsWith(prefix)) continue;
        const value = trimmed.slice(prefix.length).trim();
        if (value) return value;
    }
    return null;
}

function queryMachineGuid() {
    const output = runCommand('reg', [
        'query',
        'HKLM\\SOFTWARE\\Microsoft\\Cryptography',
        '/v',
        'MachineGuid',
    ]);
    if (!output) return null;
    const match = /MachineGuid\s+REG_SZ\s+(.+)/i.exec(output);
    return match ? match[1].trim() : null;
}

function getUnityDeviceId() {
    if (_unityDeviceId !== undefined) return _unityDeviceId || null;

    const parts = [];
    for (const [className, field] of [
        ['Win32_BaseBoard', 'SerialNumber'],
        ['Win32_BIOS', 'SerialNumber'],
        ['Win32_OperatingSystem', 'SerialNumber'],
    ]) {
        const value = queryPowerShell(className, field) || queryWmic(className, field);
        if (value) parts.push(value);
    }

    if (parts.length === 0) {
        _unityDeviceId = queryMachineGuid();
    } else {
        _unityDeviceId = crypto.createHash('sha1').update(parts.join(''), 'utf8').digest('hex');
    }
    return _unityDeviceId || null;
}

function defaultDevice() {
    return envValue('DEVICE') || getUnityDeviceId();
}

function parseYoSdkUserInfo(file) {
    const data = readJsonFile(file);
    if (!data) {
        console.error(`${file}: empty or invalid JSON`);
        return [];
    }

    let mtime = 0;
    try {
        mtime = fs.statSync(file).mtimeMs;
    } catch (_) {
    }

    const region = yoSdkRegionFromFile(file);
    const users = [];
    let skippedMissingUid = 0;
    let skippedMissingToken = 0;
    let skippedMissingDevice = 0;

    const historyValue = data.all_user_login_history;
    const history = parseJsonValue(historyValue);
    if (Array.isArray(history)) {
        const entries = history
            .map(parseJsonValue)
            .filter(Boolean)
            .sort((a, b) => Number(b.loginTime || 0) - Number(a.loginTime || 0));
        const currentUid = stringValue(data.ID);
        const currentToken = stringValue(data.Token);
        const fallbackDevice = entries.map((entry) => stringValue(entry.DeviceID)).find(Boolean)
            || getUnityDeviceId();

        for (const entry of entries) {
            const uid = stringValue(entry.ID);
            if (!uid) {
                skippedMissingUid += 1;
                continue;
            }
            const token = stringValue(entry.Token)
                || (uid === currentUid ? currentToken : null);
            if (!token) {
                skippedMissingToken += 1;
                continue;
            }
            const device = stringValue(entry.DeviceID) || fallbackDevice;
            if (!device) {
                skippedMissingDevice += 1;
                continue;
            }

            users.push({
                uid,
                token,
                device,
                file,
                region,
                mtime,
                loginTime: Number(entry.loginTime || 0),
            });
        }
    } else if (historyValue !== undefined) {
        console.error(`${file}: all_user_login_history is not a valid array`);
    }

    const userInfoValue = data.UserInfo;
    const userInfo = parseJsonValue(userInfoValue);
    if (userInfo && typeof userInfo === 'object' && !Array.isArray(userInfo)) {
        const uid = stringValue(userInfo.ID) || stringValue(userInfo.PassId);
        const token = stringValue(userInfo.Token);
        const device = stringValue(userInfo.DeviceID) || getUnityDeviceId();
        const missing = [];
        if (!uid) missing.push('UID');
        if (!token) missing.push('Token');
        if (!device) missing.push('DeviceID');

        if (uid && token && device) {
            users.push({
                uid,
                token,
                device,
                file,
                region,
                mtime,
                loginTime: 0,
            });
        } else {
            console.error(`${file}: UserInfo skipped (missing ${missing.join(', ')})`);
        }
    } else if (userInfoValue !== undefined) {
        console.error(`${file}: invalid UserInfo JSON`);
    }

    if (skippedMissingUid + skippedMissingToken + skippedMissingDevice > 0) {
        console.error(`${file}: skipped login history entries (missing UID=${skippedMissingUid}, token=${skippedMissingToken}, DeviceID=${skippedMissingDevice})`);
    }

    if (users.length === 0) {
        const keys = Object.keys(data);
        console.error(`${file}: no usable account (top-level keys: ${keys.length ? keys.join(', ') : 'none'})`);
    }

    return users;
}

let _yoSdkCache = null;

function loadYoSdkCredentials() {
    if (_yoSdkCache) return _yoSdkCache;

    const files = findYoSdkFiles();
    const users = [];
    for (const file of files) {
        if (!isCredentialFile(file)) {
            console.error(`Skipping ${file} (not a credential file)`);
            continue;
        }

        const parsed = parseYoSdkUserInfo(file);
        users.push(...parsed);
    }
    users.sort((a, b) => b.mtime - a.mtime || b.loginTime - a.loginTime);

    const byRegion = new Map();
    for (const user of users) {
        if (!user.region) {
            console.error(`Skipping ${user.file}: could not determine region from filename`);
            continue;
        }
        if (!byRegion.has(user.region)) byRegion.set(user.region, new Map());
        const accounts = byRegion.get(user.region);
        const existing = accounts.get(user.uid);
        if (!existing) {
            accounts.set(user.uid, user);
            continue;
        }
        const newer = user.mtime > existing.mtime
            || (user.mtime === existing.mtime && user.loginTime > existing.loginTime);
        if (newer) {
            accounts.set(user.uid, user);
        }
    }

    if (byRegion.size === 0) {
        console.error('No regions detected from credential files');
    }
    _yoSdkCache = new Map();
    for (const [region, accounts] of byRegion) {
        _yoSdkCache.set(region, [...accounts.values()].sort((a, b) => compareUids(a.uid, b.uid)));
    }
    return _yoSdkCache;
}

function findYoSdkCredentials(regionKey) {
    return loadYoSdkCredentials().get(regionKey) || [];
}

function regionCredentials(region, auto) {
    if (!auto) return null;
    console.log(`Using ${auto.file} (${region.label}, Yostar UID ${auto.uid})`);
    return {
        token: auto.token,
        uid: auto.uid,
        device: auto.device,
    };
}

function encryptGCM(plaintext, key, useAad = true) {
    const iv = crypto.randomBytes(12);
    const cipher = crypto.createCipheriv('aes-256-gcm', key, iv, { authTagLength: 16 });
    if (useAad) cipher.setAAD(iv);
    const ciphertext = Buffer.concat([cipher.update(plaintext), cipher.final()]);
    return Buffer.concat([iv, ciphertext, cipher.getAuthTag()]);
}

function decryptGCM(data, key, useAad = true) {
    const iv = data.slice(0, 12);
    const enc = data.slice(12);
    if (enc.length < 16) throw new Error('Invalid GCM payload');
    const tag = enc.slice(enc.length - 16);
    const decipher = crypto.createDecipheriv('aes-256-gcm', key, iv, { authTagLength: 16 });
    if (useAad) decipher.setAAD(iv);
    decipher.setAuthTag(tag);
    return Buffer.concat([decipher.update(enc.slice(0, enc.length - 16)), decipher.final()]);
}

function encryptChaCha(plaintext, key, useAad = true) {
    const iv = crypto.randomBytes(12);
    const cipher = crypto.createCipheriv('chacha20-poly1305', key, iv, { authTagLength: 16 });
    if (useAad) cipher.setAAD(iv);
    const ciphertext = Buffer.concat([cipher.update(plaintext), cipher.final()]);
    return Buffer.concat([iv, ciphertext, cipher.getAuthTag()]);
}

function decryptChaCha(data, key, useAad = true) {
    const iv = data.slice(0, 12);
    const enc = data.slice(12);
    if (enc.length < 16) throw new Error('Invalid ChaCha payload');
    const tag = enc.slice(enc.length - 16);
    const decipher = crypto.createDecipheriv('chacha20-poly1305', key, iv, { authTagLength: 16 });
    if (useAad) decipher.setAAD(iv);
    decipher.setAuthTag(tag);
    return Buffer.concat([decipher.update(enc.slice(0, enc.length - 16)), decipher.final()]);
}

function encryptBasic(buf, key) {
    const out = Buffer.from(buf);
    for (let i = 0; i < out.length; i++) {
        out[i] = out[i] ^ key[i % key.length];
        const b = out[i] & 0xff;
        const v7 = (b << 1) & 0xff;
        const v1 = (b >> 7) & 0x01;
        out[i] = (v1 | v7) & 0xff;
        out[i] = out[i] ^ out.length;
    }
    return out;
}

function decryptBasic(buf, key) {
    const out = Buffer.from(buf);
    for (let i = 0; i < out.length; i++) {
        out[i] = out[i] ^ out.length;
        const b = out[i] & 0xff;
        const v1 = (b << 7) & 0xff;
        const v7 = (b >> 1) & 0x7f;
        out[i] = (v7 | v1) & 0xff;
        out[i] = out[i] ^ key[i % key.length];
    }
    return out;
}

function hkdfSha256(ikm, salt, info, length) {
    const prk = crypto.createHmac('sha256', salt).update(ikm).digest();
    const hashLen = 32;
    const n = Math.ceil(length / hashLen);
    let prev = Buffer.alloc(0);
    const out = [];
    for (let i = 1; i <= n; i++) {
        const hmac = crypto.createHmac('sha256', prk);
        hmac.update(prev);
        hmac.update(info);
        hmac.update(Buffer.from([i]));
        prev = hmac.digest();
        out.push(prev);
    }
    return Buffer.concat(out).slice(0, length);
}

function generateSessionKey(sharedKey, serverPub, clientPub) {
    const ikm = Buffer.alloc(32);
    const count = Math.min(sharedKey.length, 32);
    sharedKey.copy(ikm, 32 - count, 0, count);

    const salt = Buffer.from(serverPub);
    const srvLen = salt.length;
    const clientBuf = Buffer.from(clientPub);
    const info = Buffer.alloc(clientBuf.length);
    for (let i = 0; i < clientBuf.length; i++) {
        const c = clientBuf[i] & 0xff;
        let s = salt[i % srvLen] & 0xff;
        if (c > s) s = (s << 1) & 0xff;
        else s = (s >> 1) & 0xff;
        info[i] = (s ^ c) & 0xff;
    }
    return hkdfSha256(ikm, salt, info, 32);
}

let _serverTs = null;
let _clientSyncTime = 0;
let _httpSeq = 0;

function markServerTime(serverTs) {
    _serverTs = BigInt(serverTs.toString());
    _clientSyncTime = Math.floor(process.uptime());
}

function makeHeader10() {
    const buf = Buffer.alloc(10, 0);
    const now = _serverTs === null
        ? BigInt(Math.floor(Date.now() / 1000))
        : _serverTs + BigInt(Math.floor(process.uptime())) - BigInt(_clientSyncTime);
    let value = now < 0n ? (1n << 64n) + now : now;
    for (let i = 0; i < 8; i++) {
        buf[7 - i] = Number(value & 0xffn);
        value >>= 8n;
    }
    buf.writeUInt16BE(_httpSeq & 0xffff, 8);
    _httpSeq = (_httpSeq + 1) & 0xffff;
    return buf;
}

function buildNovaMessage(msgId, body, cipher, sessionKey, garbleKey) {
    const packet = Buffer.alloc(2 + body.length);
    packet.writeUInt16BE(msgId, 0);
    if (body.length) body.copy(packet, 2);
    const plaintext = Buffer.concat([makeHeader10(), packet]);
    const useGarble = msgId === 1;
    const key = useGarble ? garbleKey : sessionKey;
    let enc = cipher === 1
        ? encryptChaCha(plaintext, key)
        : encryptGCM(plaintext, key);
    if (useGarble) enc = encryptBasic(enc, garbleKey);
    return enc;
}

function decodeNovaMessage(buf, cipher, sessionKey) {
    return cipher === 1
        ? decryptChaCha(buf, sessionKey)
        : decryptGCM(buf, sessionKey);
}

async function postBuffer(url, buf, headers = {}) {
    const res = await fetch(url, {
        method: 'POST',
        headers: Object.assign({ 'Content-Type': 'application/octet-stream' }, headers),
        body: buf,
    });
    if (!res.ok) {
        const detail = await res.text().catch(() => '');
        throw new Error(`POST ${url} failed with ${res.status} ${res.statusText}${detail ? `: ${detail}` : ''}`);
    }
    return Buffer.from(await res.arrayBuffer());
}

async function doIkeHandshake(region) {
    const ecdh = crypto.createECDH('prime256v1');
    const clientPub = ecdh.generateKeys();
    const req = IKEReq.encode(IKEReq.create({
        ProtoVersion: 1,
        ClientTs: Math.floor(Date.now() / 1000),
        PubKey: clientPub,
    })).finish();
    const packet = Buffer.alloc(2 + req.length);
    packet.writeUInt16BE(1, 0);
    req.copy(packet, 2);

    const body = encryptBasic(encryptGCM(packet, region.garbleKey), region.garbleKey);
    const respBuf = await postBuffer(`${region.serverUrl}/agent-zone-1/`, body);
    const plain = decryptGCM(decryptBasic(respBuf, region.garbleKey), region.garbleKey);
    const msgId = plain.readUInt16BE(0);
    if (msgId !== 2) throw new Error(`Unexpected IKE response msgId: ${msgId}`);

    const resp = IKEResp.decode(plain.slice(2));
    if (resp.ServerTs !== undefined && resp.ServerTs !== null) markServerTime(resp.ServerTs);
    if (!resp.PubKey || resp.PubKey.length === 0) throw new Error('IKE response has no server public key');

    const shared = ecdh.computeSecret(Buffer.from(resp.PubKey));
    const sessionKey = generateSessionKey(shared, Buffer.from(resp.PubKey), clientPub);
    return { token: resp.Token, cipher: resp.Cipher, sessionKey };
}

const PRIVATE_KEY_CN = `-----BEGIN RSA PRIVATE KEY-----
MIICXQIBAAKBgQDBdHUbBS5ZEnYbmLzUagVUD+WnG3h7vtPIb6lxLsIONtE35JEI
PY3Q6+Klv+mMAR2f8yoxp/OAD2QGwOhpMVT7HK8HyDN6W4mjx6D61/5k3FhHRfok
Fk4pyJWxCiccqO6tzMgbyqNB8W2vH9qryGdKKmjoeKfyhCKjfI5mI5QSrQIDAQAB
AoGBALUUrn0xV1X26ukgCjkGWKB8FOpb02Z5lEG9C60vKGQnuaUI5R0CiRmzgz7F
KfZsIO/UrH04ibwoK+gKcEHu9GxL0+qjqoq1KgNRhy3nqSqk1EhPmLxfUaaCdGBB
I5p4Bb8RAvtCwRABzkYobeZBQcOwSa/az0lpx4+IJu1wZAsdAkEA7RVtp6ia/QAO
JWg2ikQpYDAWB7XtVb8Q/kpGodkLCDbJbxS9+6iXHpnjoGQicwR/AnIqWyp8liPC
6agU4hoGowJBANDj4sQj+F8l24KjM04aTf0V+NCXr0rhjvKNm1GEIhz56iNxlq5O
dMcsv9B88puwvDMN7pvjm/4O3Pgt6tVlJm8CQHa9L7EzoYQq3ergOcigALT7AF2W
QSqbyfAB1UREBuFzRwmKMuhydFVQL4/p9tLyIoZgFVSZf9JWUM1jcCoZbM8CQBc+
A/FG/0+26qGyfvblvl+2niS3e+2dA0bBstJzHUiUFhUzgkyKtjujIwSTonroQ0h6
+F2oCadPCA8lLMfppUkCQQCAUYlW1Ys3DgUi+3gDZbhWqQajojfYU5PbhxhlZwc5
2TnwB+07yrrBu1yzzb0LbDWSrJUlNL2UZ438GGqrWJ0l
-----END RSA PRIVATE KEY-----`;

function generateYostarAuthHeaderCN(region, credentials, head, body) {
    const HEAD = Object.assign({
        Channel: 'official',
        Platform: 'pc',
        Lang: region.authLang,
        DeviceID: credentials.device,
        Version: region.sdkVersion,
        GVersionNo: CLIENT_VERSION,
        GBuildNo: '',
        PID: region.pid,
        DeviceModel: DEVICE_MODEL,
        Time: Math.floor(Date.now() / 1000),
        UID: Number(credentials.uid),
        Token: credentials.token,
        RID: crypto.randomUUID(),
    }, head);

    const toSign = JSON.stringify(HEAD) + JSON.stringify(body);
    const signer = crypto.createSign('RSA-SHA256');
    signer.update(toSign, 'utf8');
    const sign = signer.sign(PRIVATE_KEY_CN).toString('base64');
    return JSON.stringify({ Head: HEAD, Sign: sign });
}

function generateYostarAuthHeader(region, credentials, head, body) {
    if (region.authMode === 'cn') {
        return generateYostarAuthHeaderCN(region, credentials, head, body);
    }

    const SALT = '886c085e4a8d30a703367b120dd8353948405ec2';
    const HEAD = {
        Channel: 'pc',
        Platform: 'pc',
        Lang: region.authLang,
        DeviceID: credentials.device,
        Version: region.sdkVersion,
        GVersionNo: CLIENT_VERSION,
        GBuildNo: '',
        PID: region.pid,
        DeviceModel: DEVICE_MODEL,
        Time: Math.floor(Date.now() / 1000),
    };
    if (credentials.uid) HEAD.UID = credentials.uid;
    HEAD.Token = '';
    Object.assign(HEAD, head);

    const toSign = JSON.stringify(HEAD) + JSON.stringify(body) + SALT;
    const sign = crypto.createHash('md5').update(toSign, 'utf8').digest('hex').toUpperCase();
    return JSON.stringify({ Head: HEAD, Sign: sign });
}

function extractSdkUser(obj) {
    const data = obj && obj.Data;
    return data && (data.User || data.UserInfo);
}

async function quickLogin(region, credentials, savedToken) {
    const url = `${region.sdkUrl}/user/quick-login`;
    const bodyObj = {};
    const head = region.authMode === 'cn' ? {} : { Token: savedToken };
    const authHeader = generateYostarAuthHeader(region, credentials, head, bodyObj);
    const respBuf = await postBuffer(url, Buffer.from('{}'), {
        'Content-Type': 'application/json',
        Authorization: authHeader,
    });
    const text = respBuf.toString('utf8');
    let obj;
    try {
        obj = JSON.parse(text);
    } catch (_) {
        throw new Error(`/user/quick-login returned invalid JSON: ${text.slice(0, 300)}`);
    }
    const user = extractSdkUser(obj);
    if (!obj || obj.Code !== 200 || !user || !user.Token) {
        throw new Error(`/user/quick-login failed: ${text.slice(0, 500)}`);
    }
    return {
        accountLoginToken: user.Token,
        accountUid: user.ID,
    };
}

async function getAccountLoginToken(region, credentials) {
    console.log('Found saved Yostar token; trying quick-login...');
    return quickLogin(region, credentials, credentials.token);
}

function ask(promptText) {
    const rl = readline.createInterface({ input: process.stdin, output: process.stdout });
    return new Promise((resolve) => {
        rl.question(promptText, (answer) => {
            rl.close();
            resolve(answer.trim());
        });
    });
}

async function selectRegion() {
    if (!process.stdin.isTTY || !process.stdout.isTTY) {
        throw new Error('No usable credentials. Run in a terminal to log in');
    }

    const loginRegions = REGION_ORDER
        .map((regionKey) => REGIONS[regionKey])
        .filter((region) => region.authMode !== 'cn');

    console.log('Select a region:');
    loginRegions.forEach((region, index) => {
        console.log(`  ${index + 1}. ${region.label}`);
    });
    const input = await ask(`Region [1-${loginRegions.length}]: `);
    const regionKey = normalizeRegion(input);
    if (regionKey && loginRegions.some((region) => region.key === regionKey)) {
        return REGIONS[regionKey];
    }
    if (!/^\d+$/.test(input)) throw new Error('Invalid region selection');
    const index = Number(input);
    if (index < 1 || index > loginRegions.length) throw new Error('Invalid region selection');
    return loginRegions[index - 1];
}

function loginCredentials(device, uid = '') {
    return { uid: uid || '', token: '', device };
}

async function postSdkJson(region, endpoint, bodyObj, device, uid = '') {
    const url = `${region.sdkUrl}${endpoint}`;
    const credentials = loginCredentials(device, uid);
    const authHeader = generateYostarAuthHeader(region, credentials, {}, bodyObj);
    const respBuf = await postBuffer(url, Buffer.from(JSON.stringify(bodyObj), 'utf8'), {
        'Content-Type': 'application/json',
        Authorization: authHeader,
    });
    const text = respBuf.toString('utf8');
    let obj;
    try {
        obj = JSON.parse(text);
    } catch (_) {
        throw new Error(`${endpoint} returned invalid JSON: ${text.slice(0, 300)}`);
    }
    return { obj, text };
}

async function sendAuthCodeRequest(region, email, device, uid = '') {
    const { obj, text } = await postSdkJson(region, '/yostar/send-code', { Account: email }, device, uid);
    if (!obj || obj.Code !== 200) {
        throw new Error(`/yostar/send-code failed: ${text.slice(0, 500)}`);
    }
}

async function loginWithEmailCode(region, email, code, device, uid = '') {
    const auth = await postSdkJson(region, '/yostar/get-auth', { Account: email, Code: code }, device, uid);
    const authToken = auth.obj && auth.obj.Data && auth.obj.Data.Token;
    if (!authToken) {
        throw new Error(`/yostar/get-auth failed: ${auth.text.slice(0, 500)}`);
    }

    const login = await postSdkJson(region, '/user/login', {
        OpenID: email,
        Token: authToken,
        Type: 'yostar',
        UserName: email,
        Secret: '',
        CheckAccount: 0,
    }, device, uid);
    const user = extractSdkUser(login.obj);
    if (!user || !user.Token || user.ID === undefined || user.ID === null || user.ID === '') {
        throw new Error(`/user/login failed: ${login.text.slice(0, 500)}`);
    }

    return { accountLoginToken: user.Token, accountUid: user.ID };
}

async function trySavedLogin(region) {
    const saved = loadSavedLogin(region.key);
    if (!saved) return null;

    const device = saved.device || defaultDevice();
    if (!device) {
        console.error(`Saved login failed (${region.label}): could not determine device ID`);
        return null;
    }

    const credentials = {
        uid: saved.uid,
        token: saved.token,
        device,
    };
    try {
        const quick = await getAccountLoginToken(region, credentials);
        console.log(`Using saved login (${region.label}, Yostar UID ${quick.accountUid})`);
        return { credentials, quick };
    } catch (err) {
        console.error(`Saved login failed (${region.label}): ${err.message}`);
        return null;
    }
}

async function loginWithEmail(region) {
    if (!process.stdin.isTTY || !process.stdout.isTTY) {
        throw new Error('Email login requires an interactive terminal');
    }

    const device = defaultDevice();
    if (!device) throw new Error('Could not determine device ID');
    const saved = loadSavedLogin(region.key);
    const uid = saved ? saved.uid : '';
    const email = await ask(`Yostar email (${region.label}): `);
    if (!email) throw new Error('Email is required');
    console.log('Requesting Yostar code...');
    await sendAuthCodeRequest(region, email, device, uid);
    const code = await ask(`Enter code (${region.label}): `);
    if (!code) throw new Error('Code is required');

    const quick = await loginWithEmailCode(region, email, code, device, uid);
    saveLogin(region.key, quick.accountUid, quick.accountLoginToken, device);
    console.log(`Logged in (${region.label}, Yostar UID ${quick.accountUid})`);
    const credentials = {
        uid: String(quick.accountUid),
        token: quick.accountLoginToken,
        device,
    };
    return { credentials, quick };
}

async function doPlayerLogin(region, credentials, ikeToken, cipher, sessionKey, accountLoginToken, accountUid) {
    const login = {
        Platform: 3,
        Language: region.language,
        Channel: 'Official',
        Device: credentials.device,
        Version: CLIENT_VERSION,
    };
    login[region.loginField] = {
        Uid: accountUid,
        Token: accountLoginToken,
    };
    const req = LoginReq.create(login);
    const body = LoginReq.encode(req).finish();
    const payload = buildNovaMessage(MSG.loginReq, body, cipher, sessionKey, region.garbleKey);
    const respBuf = await postBuffer(`${region.serverUrl}/agent-zone-1/`, payload, {
        'X-Token': ikeToken,
    });
    const plain = decodeNovaMessage(respBuf, cipher, sessionKey);
    const msgId = plain.readUInt16BE(0);

    if (msgId === MSG.loginSucceedAck) {
        return LoginResp.decode(plain.slice(2));
    }
    if (msgId === MSG.systemFailedAck || msgId === MSG.loginFailedAck) {
        let detail;
        try {
            detail = ErrorType.decode(plain.slice(2));
        } catch (_) {
            detail = {};
        }
        throw new Error(`Player login failed (msgId ${msgId}): ${JSON.stringify(detail)}`);
    }
    throw new Error(`Unexpected player login response msgId: ${msgId}`);
}

async function sendGameRequest(region, token, cipher, sessionKey, msgId, body, expectedAck) {
    const payload = buildNovaMessage(msgId, body, cipher, sessionKey, region.garbleKey);
    const respBuf = await postBuffer(`${region.serverUrl}/agent-zone-1/`, payload, {
        'X-Token': token,
    });
    const plain = decodeNovaMessage(respBuf, cipher, sessionKey);
    const respMsgId = plain.readUInt16BE(0);
    if (respMsgId === expectedAck) return plain.slice(2);
    if (respMsgId === MSG.systemFailedAck) {
        let detail;
        try {
            detail = ErrorType.decode(plain.slice(2));
        } catch (_) {
            detail = {};
        }
        throw new Error(`Server returned system_failed_ack: ${JSON.stringify(detail)}`);
    }
    throw new Error(`Expected ack ${expectedAck}, got ${respMsgId}`);
}

async function withRetry(fn, attempts = 3, delayMs = 800) {
    let lastError;
    for (let attempt = 1; attempt <= attempts; attempt++) {
        try {
            return await fn();
        } catch (err) {
            lastError = err;
            if (attempt < attempts) await new Promise((resolve) => setTimeout(resolve, delayMs * attempt));
        }
    }
    throw lastError;
}


async function fetchJson(url, label) {
    let lastError;
    for (let attempt = 1; attempt <= 3; attempt++) {
        try {
            const res = await fetch(url);
            if (!res.ok) throw new Error(`${res.status} ${res.statusText}`);
            return await res.json();
        } catch (err) {
            lastError = err;
            if (attempt < 3) await new Promise((resolve) => setTimeout(resolve, 500 * attempt));
        }
    }
    throw new Error(`Failed to fetch ${label} (${url}): ${lastError.message}`);
}

async function loadMetadata(region) {
    console.log('Fetching stelladb data...');
    const [rawBanners, rawStorages, bannerLang, storageLang, extraBanners, itemData] = await Promise.all([
        fetchJson(DATA_URLS.banners(region), `${region.dataPath}/bin/Gacha.json`),
        fetchJson(DATA_URLS.storages(region), `${region.dataPath}/bin/GachaStorage.json`),
        fetchJson(DATA_URLS.bannerLang(region), `${region.dataPath}/language/${region.language}/Gacha.json`),
        fetchJson(DATA_URLS.storageLang(region), `${region.dataPath}/language/${region.language}/GachaStorage.json`),
        fetchJson(DATA_URLS.gacha, 'gacha.json'),
        fetchJson(DATA_URLS.items, 'item.json'),
    ]);
    return { rawBanners, rawStorages, bannerLang, storageLang, extraBanners, itemData };
}

function basename(value) {
    if (!value || typeof value !== 'string') return null;
    return value.split('/').pop();
}

function storageKeyFromName(name) {
    if (!name || typeof name !== 'string') return null;
    const parts = name.split('.');
    return parts.length >= 2 ? parts.slice(1, -1).join('.') : name;
}

function buildCatalog(metadata) {
    const { rawBanners, rawStorages, bannerLang, storageLang, extraBanners } = metadata;
    const extraById = extraBanners;

    const storages = new Map();
    for (const [id, raw] of Object.entries(rawStorages)) {
        const key = storageKeyFromName(raw.Name);
        const name = storageLang[raw.Name] || key || `Pool ${id}`;
        const type = /disc/i.test(key) ? 'disc' : 'character';
        storages.set(Number(id), {
            id: Number(id),
            key,
            name,
            type,
        });
    }

    const banners = [];
    const bannerById = new Map();
    for (const [idText, raw] of Object.entries(rawBanners)) {
        const id = Number(idText);
        const extra = extraById[idText];
        const storageId = Number(raw.StorageId);
        const storage = storages.get(storageId);
        const name = bannerLang[raw.Name] || extra?.name || `Banner ${id}`;
        const type = extra?.type || storage.type;
        const icon = extra?.icon || basename(raw.Icon);
        const banner = {
            id,
            storageId,
            storageKey: storage.key,
            storageName: storage.name,
            type,
            name,
            icon,
            image: raw.Image,
            startTime: raw.StartTime,
            endTime: raw.EndTime,
            active: isBannerActive(raw.StartTime, raw.EndTime),
            rateUp5Star: extra?.rateUp5Star || [],
            rateUp4Star: extra?.rateUp4Star || [],
        };
        banners.push(banner);
        bannerById.set(id, banner);
    }

    return { banners, bannerById, storages };
}

function isBannerActive(startTime, endTime) {
    const now = Date.now();
    if (startTime) {
        const start = Date.parse(startTime);
        if (Number.isFinite(start) && now < start) return false;
    }
    if (endTime) {
        const end = Date.parse(endTime);
        if (Number.isFinite(end) && now > end) return false;
    }
    return true;
}


function pullKey(pull) {
    return `${pull.bannerId}|${pull.storageId}|${pull.time}|${pull.ids.join(',')}`;
}

function mergePulls(existingPulls, freshPulls) {
    if (!Array.isArray(existingPulls) || existingPulls.length === 0) return freshPulls;

    const remaining = new Map();
    for (const pull of existingPulls) {
        if (!Array.isArray(pull.ids)) continue;
        const key = pullKey(pull);
        remaining.set(key, (remaining.get(key) || 0) + 1);
    }

    const merged = [];
    for (const pull of freshPulls) {
        merged.push(pull);
        const key = pullKey(pull);
        const count = remaining.get(key) || 0;
        if (count > 0) remaining.set(key, count - 1);
    }
    for (const pull of existingPulls) {
        if (!Array.isArray(pull.ids)) continue;
        const key = pullKey(pull);
        const count = remaining.get(key) || 0;
        if (count > 0) {
            merged.push(pull);
            remaining.set(key, count - 1);
        }
    }
    return merged;
}

async function fetchGameIdentity(region, sessionToken, cipher, sessionKey) {
    const respBuf = await withRetry(() => sendGameRequest(
        region, sessionToken, cipher, sessionKey,
        MSG.playerDataReq, Buffer.alloc(0), MSG.playerDataSucceedAck,
    ));
    const player = PlayerInfo.decode(respBuf);
    const gameUid = stringValue(player.Acc?.Id);
    if (!gameUid || gameUid === '0') throw new Error('Player data has no game UID');
    const gameName = stringValue(player.Acc?.NickName);
    return { gameUid, gameName };
}

async function fetchRegionHistory(region, credentials, quick, metadata) {
    const { banners, bannerById, storages } = buildCatalog(metadata);

    console.log(`Connecting to ${region.serverUrl}...`);
    const ike = await doIkeHandshake(region);
    console.log(`IKE ok (${ike.cipher === 1 ? 'ChaCha20-Poly1305' : 'AES-GCM'})`);

    const loginResp = await doPlayerLogin(
        region, credentials, ike.token, ike.cipher, ike.sessionKey,
        quick.accountLoginToken, quick.accountUid,
    );
    const sessionToken = loginResp.Token;
    if (!sessionToken) throw new Error('Player login returned an empty session token');

    _httpSeq = 1;

    const { gameUid, gameName } = await fetchGameIdentity(
        region, sessionToken, ike.cipher, ike.sessionKey,
    );
    console.log(gameName
        ? `Name: ${gameName} (game UID ${gameUid})`
        : `Game UID ${gameUid}`);

    const storageIds = [...new Set([
        ...storages.keys(),
        ...banners.map((banner) => banner.storageId),
    ])].filter((id) => Number.isFinite(id) && id > 0).sort((a, b) => a - b);

    const pulls = [];
    const errors = [];

    for (const storageId of storageIds) {
        try {
            const body = UI32.encode(UI32.create({ Value: storageId })).finish();
            const respBuf = await withRetry(() => sendGameRequest(
                region, sessionToken, ike.cipher, ike.sessionKey,
                MSG.gachaHistoriesReq, body, MSG.gachaHistoriesSucceedAck,
            ));
            const histories = GachaHistories.decode(respBuf);
            const list = histories.List;
            for (const history of list) {
                if (!history || !Array.isArray(history.Ids)) continue;
                pulls.push({
                    bannerId: Number(history.Gid),
                    storageId,
                    time: Number(history.Time),
                    ids: history.Ids.map(Number),
                });
            }
        } catch (err) {
            errors.push({ storageId, error: err.message });
            console.error(`  storage ${storageId}: ${err.message}`);
        }
    }

    let gachaInfo = [];
    try {
        const respBuf = await withRetry(() => sendGameRequest(
            region, sessionToken, ike.cipher, ike.sessionKey,
            MSG.gachaInformationReq, Buffer.alloc(0), MSG.gachaInformationSucceedAck,
        ));
        gachaInfo = GachaInformationResp.decode(respBuf).Information;
    } catch (err) {
        errors.push({ storageId: 'information', error: err.message });
        console.warn(`Gacha information failed: ${err.message}`);
    }

    return {
        uid: String(quick.accountUid),
        gameUid,
        gameName,
        banners,
        bannerById,
        storages,
        pulls,
        gachaInfo,
        errors,
    };
}


function zeroCounts() {
    return { pulls: 0, items: 0, rateUpItems: 0, rateUpSSR: 0, byRarity: {}, byType: {} };
}

function isSparkable(banner) {
    return banner.type === 'character' && Boolean(banner.rateUp5Star?.length);
}

function buildSparkInfo(banner, stats, pity) {
    const sparkable = isSparkable(banner);
    const bannerPulls = pity ? pity.gachaTotalTimes : null;
    const countedPulls = sparkable
        ? Math.max(stats.items, bannerPulls || 0)
        : stats.items;
    return {
        sparkable,
        threshold: SPARK_THRESHOLD,
        countedPulls,
        historyItems: stats.items,
        bannerPulls,
        sparked: sparkable && countedPulls >= SPARK_THRESHOLD,
    };
}

function addCounts(target, item) {
    target.items += 1;
    if (item.rarity) target.byRarity[item.rarity] = (target.byRarity[item.rarity] || 0) + 1;
    if (item.type) target.byType[item.type] = (target.byType[item.type] || 0) + 1;
    if (item.rateUp) target.rateUpItems += 1;
    if (item.rateUp >= 5) target.rateUpSSR += 1;
}

function sortObjectByKey(obj) {
    return Object.fromEntries(Object.entries(obj).sort(([a], [b]) => a.localeCompare(b)));
}

function withSums(target, source) {
    target.pulls += source.pulls;
    target.items += source.items;
    target.rateUpItems += source.rateUpItems;
    target.rateUpSSR += source.rateUpSSR;
    for (const [key, value] of Object.entries(source.byRarity)) {
        target.byRarity[key] = (target.byRarity[key] || 0) + value;
    }
    for (const [key, value] of Object.entries(source.byType)) {
        target.byType[key] = (target.byType[key] || 0) + value;
    }
}

function buildRegionOutput(regionKey, fetched, existingRegion, itemData) {
    const { banners, bannerById, storages } = fetched;
    const existingMatches = Boolean(existingRegion && String(existingRegion.uid) === String(fetched.uid));

    const existingPulls = existingMatches && Array.isArray(existingRegion.history)
        ? existingRegion.history.map((entry) => ({
            bannerId: Number(entry.bannerId),
            storageId: Number(entry.storageId),
            time: Number(entry.time),
            ids: entry.ids.map(Number),
        }))
        : [];

    const pulls = mergePulls(existingPulls, fetched.pulls);

    const allBanners = banners.slice();
    for (const pull of pulls) {
        if (!bannerById.has(pull.bannerId)) {
            const storage = storages.get(pull.storageId);
            const banner = {
                id: pull.bannerId,
                storageId: pull.storageId,
                storageKey: storage?.key,
                storageName: storage?.name,
                type: storage?.type,
                name: `Unknown Banner ${pull.bannerId}`,
                icon: null,
                image: null,
                startTime: null,
                endTime: null,
                active: false,
                rateUp5Star: [],
                rateUp4Star: [],
                unknown: true,
            };
            allBanners.push(banner);
            bannerById.set(banner.id, banner);
        }
    }

    const infoById = new Map();
    for (const info of fetched.gachaInfo) {
        if (info && info.Id) infoById.set(Number(info.Id), info);
    }

    const itemsById = {};
    const perBanner = new Map();
    for (const banner of allBanners) {
        perBanner.set(banner.id, []);
    }

    const rateUp5 = new Map();
    const rateUp4 = new Map();
    for (const banner of allBanners) {
        rateUp5.set(banner.id, new Set(banner.rateUp5Star.map((entry) => Number(entry.id))));
        rateUp4.set(banner.id, new Set(banner.rateUp4Star.map((entry) => Number(entry.id))));
    }

    function getItem(id, bannerId) {
        const raw = itemData[String(id)];
        const rateUp = rateUp5.get(bannerId)?.has(id)
            ? 5
            : (rateUp4.get(bannerId)?.has(id) ? 4 : null);
        const item = {
            id,
            name: raw?.name,
            type: raw?.type,
            rarity: raw?.rarity,
        };
        if (rateUp) item.rateUp = rateUp;
        if (!itemsById[id]) itemsById[id] = { ...item };
        else if (rateUp && !itemsById[id].rateUp) itemsById[id].rateUp = rateUp;
        return item;
    }

    for (const pull of pulls) {
        const items = pull.ids.map((id) => getItem(id, pull.bannerId));
        const entry = {
            time: pull.time,
            timeISO: new Date(pull.time * 1000).toISOString(),
            ids: pull.ids,
            items,
        };
        perBanner.get(pull.bannerId).push(entry);
    }

    const totals = zeroCounts();
    const storageStats = new Map();
    for (const storage of storages.values()) {
        storageStats.set(storage.id, {
            ...storage,
            bannerIds: [],
            pulls: 0,
            items: 0,
            rateUpItems: 0,
            rateUpSSR: 0,
            byRarity: {},
            byType: {},
        });
    }

    const outputBanners = [];
    for (const banner of allBanners) {
        const history = perBanner.get(banner.id);
        history.sort((a, b) => b.time - a.time);

        const stats = zeroCounts();
        for (const entry of history) {
            stats.pulls += entry.items.length || entry.ids.length || 1;
            for (const item of entry.items) addCounts(stats, item);
        }
        stats.byRarity = sortObjectByKey(stats.byRarity);
        stats.byType = sortObjectByKey(stats.byType);
        if (history.length) {
            stats.firstPullTime = history[history.length - 1].time;
            stats.lastPullTime = history[0].time;
        } else {
            stats.firstPullTime = null;
            stats.lastPullTime = null;
        }

        const info = infoById.get(banner.id);
        const pity = info ? {
            totalTimes: Number(info.TotalTimes) || 0,
            gachaTotalTimes: Number(info.GachaTotalTimes) || 0,
            daysCount: Number(info.DaysCount) || 0,
            aupMissTimes: Number(info.AupMissTimes) || 0,
            amissTimes: Number(info.AMissTimes) || 0,
            aupGuaranteeTimes: Number(info.AupGuaranteeTimes) || 0,
            receivedFirstTenReward: Boolean(info.ReveFirstTenReward),
            receivedGuaranteeReward: Boolean(info.RecvGuaranteeReward),
        } : null;

        outputBanners.push({
            ...banner,
            stats,
            pity,
            spark: buildSparkInfo(banner, stats, pity),
            history,
        });

        withSums(totals, stats);

        const storage = storageStats.get(banner.storageId);
        if (storage) {
            storage.bannerIds.push(banner.id);
            withSums(storage, stats);
        }
    }

    for (const storage of storageStats.values()) {
        storage.byRarity = sortObjectByKey(storage.byRarity);
        storage.byType = sortObjectByKey(storage.byType);
        storage.bannerIds.sort((a, b) => a - b);
    }

    totals.byRarity = sortObjectByKey(totals.byRarity);
    totals.byType = sortObjectByKey(totals.byType);
    totals.banners = outputBanners.length;
    totals.activeBanners = outputBanners.filter((banner) => banner.active).length;
    totals.bannersWithHistory = outputBanners.filter((banner) => banner.history.length > 0).length;
    totals.sparks = outputBanners.filter((banner) => banner.spark.sparked).length;
    totals.sparkedBannerIds = outputBanners
        .filter((banner) => banner.spark.sparked)
        .map((banner) => banner.id);

    const history = pulls
        .filter((pull) => pull.time)
        .sort((a, b) => b.time - a.time || a.bannerId - b.bannerId)
        .map((pull) => ({
            bannerId: pull.bannerId,
            bannerName: bannerById.get(pull.bannerId)?.name,
            storageId: pull.storageId,
            time: pull.time,
            timeISO: new Date(pull.time * 1000).toISOString(),
            count: pull.ids.length,
            ids: pull.ids,
        }));

    outputBanners.sort((a, b) => a.storageId - b.storageId || a.id - b.id);

    return {
        region: regionKey,
        uid: fetched.uid,
        gameUid: fetched.gameUid,
        gameName: fetched.gameName || null,
        updatedAt: new Date().toISOString(),
        totals,
        storages: [...storageStats.values()].sort((a, b) => a.id - b.id),
        banners: outputBanners,
        items: Object.fromEntries(Object.entries(itemsById).sort(([a], [b]) => Number(a) - Number(b))),
        history,
        errors: fetched.errors,
    };
}

function outputFileForAccount(regionKey, gameUid) {
    const raw = stringValue(gameUid);
    if (!raw) throw new Error('Missing game UID');
    const id = raw.replace(/[^A-Za-z0-9_-]+/g, '_');
    if (!id) throw new Error('Invalid game UID');
    return path.join(__dirname, `gacha.${id}.${regionKey}.json`);
}

function loadExistingRegion(regionKey, gameUid) {
    const file = outputFileForAccount(regionKey, gameUid);
    if (!fs.existsSync(file)) return null;
    try {
        return JSON.parse(fs.readFileSync(file, 'utf8'));
    } catch (err) {
        console.warn(`Could not read existing ${path.basename(file)}: ${err.message}`);
        return null;
    }
}

async function fetchAndWrite(region, credentials, quick, metadata) {
    const regionKey = region.key;
    const fetched = await fetchRegionHistory(region, credentials, quick, metadata);
    const regionOutput = buildRegionOutput(
        regionKey,
        fetched,
        loadExistingRegion(regionKey, fetched.gameUid),
        metadata.itemData,
    );
    const outputFile = outputFileForAccount(regionKey, fetched.gameUid);
    fs.writeFileSync(outputFile, JSON.stringify(regionOutput, null, 4), { encoding: 'utf8' });
    console.log(`${region.label}: ${regionOutput.totals.pulls} pulls`);
    console.log(`Wrote ${outputFile}`);
    return regionOutput.totals.pulls;
}

function waitForExit() {
    if (!process.stdin.isTTY || !process.stdout.isTTY) {
        return;
    }

    process.stdout.write('\nPress any key to close...');

    const stdin = process.stdin;
    let rawMode = false;
    try {
        if (stdin.setRawMode) {
            stdin.setRawMode(true);
            rawMode = true;
        }
    } catch { }

    try {
        fs.readSync(0, Buffer.alloc(1), 0, 1, null);
    } catch { }

    if (rawMode) {
        try {
            stdin.setRawMode(false);
        } catch { }
    }
    process.stdout.write('\n');
}

(async () => {
    let successCount = 0;
    let totalPulls = 0;

    const detectedRegions = REGION_ORDER
        .map((regionKey) => REGIONS[regionKey])
        .filter((region) => findYoSdkCredentials(region.key).length > 0
            || loadSavedLogin(region.key) !== null);
    const noRegionsDetected = detectedRegions.length === 0;

    const detectedLabels = detectedRegions.map((region) => region.label).join(', ');
    console.log(`Detected ${detectedRegions.length} region${detectedRegions.length === 1 ? '' : 's'}:${detectedLabels ? ` ${detectedLabels}` : ''}`);
    if (noRegionsDetected) {
        console.error('No credentials detected.');
    }

    for (const region of detectedRegions) {
        const regionKey = region.key;
        const accounts = findYoSdkCredentials(regionKey);
        let metadata;
        try {
            metadata = await loadMetadata(region);
        } catch (err) {
            console.error(`${region.label} failed: ${err.message}`);
            continue;
        }

        if (accounts.length === 0) {
            const saved = await trySavedLogin(region);
            if (!saved) continue;
            try {
                const pulls = await fetchAndWrite(region, saved.credentials, saved.quick, metadata);
                successCount += 1;
                totalPulls += pulls;
            } catch (err) {
                console.error(`${region.label} failed: ${err.message}`);
            }
            continue;
        }

        let regionSuccess = false;
        for (const auto of accounts) {
            const credentials = regionCredentials(region, auto);
            if (!credentials) continue;

            try {
                const quick = await getAccountLoginToken(region, credentials);
                const pulls = await fetchAndWrite(region, credentials, quick, metadata);
                regionSuccess = true;
                successCount += 1;
                totalPulls += pulls;
            } catch (err) {
                console.error(`${region.label} failed: ${err.message}`);
            }
        }

        if (!regionSuccess && loadSavedLogin(region.key)) {
            const saved = await trySavedLogin(region);
            if (saved) {
                try {
                    const pulls = await fetchAndWrite(region, saved.credentials, saved.quick, metadata);
                    successCount += 1;
                    totalPulls += pulls;
                } catch (err) {
                    console.error(`${region.label} failed: ${err.message}`);
                }
            }
        }
    }

    if (noRegionsDetected && successCount === 0) {
        const region = await selectRegion();
        const saved = await trySavedLogin(region);
        const { credentials, quick } = saved || await loginWithEmail(region);
        const metadata = await loadMetadata(region);
        const pulls = await fetchAndWrite(region, credentials, quick, metadata);
        successCount += 1;
        totalPulls += pulls;
    }

    if (successCount === 0) {
        throw new Error('No region could be fetched');
    }

    if (totalPulls === 0) {
        console.warn('Warning: the fetched history is empty. The metadata was still written.');
    }
})().catch((err) => {
    console.error('ERROR:', err);
    process.exitCode = 1;
}).finally(waitForExit);
