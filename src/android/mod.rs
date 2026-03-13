use crate::app::{AppOptions, BuzzClawApp};
use crate::config;
use crate::tools::external_cmd::{
    set_external_cmd_runner, ExternalCmdKind, ExternalCmdResult, ExternalCmdRunner,
};
use crate::tools::external_tools::{
    set_external_tool_runner, ExternalToolResult, ExternalToolRunner,
};
use anyhow::Result;
use jni::objects::{GlobalRef, JClass, JObject, JString};
use jni::sys::{jboolean, jlong, jstring, JNI_FALSE, JNI_TRUE};
use jni::{JNIEnv, JavaVM};
use serde::Deserialize;
use std::sync::{Arc, Mutex, OnceLock};

struct JniCallbacks {
    jvm: JavaVM,
    callback: GlobalRef,
}

impl JniCallbacks {
    fn with_env<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&mut JNIEnv<'_>) -> Result<T>,
    {
        let mut env = self.jvm.attach_current_thread()?;
        f(&mut env)
    }

    fn call_run_command(
        &self,
        kind: ExternalCmdKind,
        command: &str,
        working_dir: Option<&str>,
    ) -> Result<String> {
        self.with_env(|env| {
            let kind_int = match kind {
                ExternalCmdKind::Shell => 0,
                ExternalCmdKind::Adb => 1,
            } as i32;
            let cmd = env.new_string(command)?;
            let wd = match working_dir {
                Some(dir) => env.new_string(dir)?.into(),
                None => JObject::null(),
            };
            let res = env.call_method(
                self.callback.as_obj(),
                "runCommand",
                "(ILjava/lang/String;Ljava/lang/String;)Ljava/lang/String;",
                &[kind_int.into(), (&cmd).into(), (&wd).into()],
            )?;
            let obj = res.l()?;
            let out: String = env.get_string(&JString::from(obj))?.into();
            Ok(out)
        })
    }
}

#[derive(Deserialize)]
struct ExternalCmdJson {
    success: bool,
    output: String,
}

impl ExternalCmdRunner for JniCallbacks {
    fn run(&self, kind: ExternalCmdKind, command: &str, working_dir: Option<&str>) -> Result<ExternalCmdResult> {
        let raw = self.call_run_command(kind, command, working_dir)?;
        let parsed: ExternalCmdJson = serde_json::from_str(&raw)
            .map_err(|e| anyhow::anyhow!("invalid external cmd JSON: {e}"))?;
        Ok(ExternalCmdResult { success: parsed.success, output: parsed.output })
    }
}

impl ExternalToolRunner for JniCallbacks {
    fn web_fetch(&self, url: &str, max_chars: usize) -> Result<ExternalToolResult> {
        let raw = self.with_env(|env| {
            let url_js = env.new_string(url)?;
            let max = max_chars as i32;
            let res = env.call_method(
                self.callback.as_obj(),
                "webFetch",
                "(Ljava/lang/String;I)Ljava/lang/String;",
                &[(&url_js).into(), max.into()],
            )?;
            let obj = res.l()?;
            let out: String = env.get_string(&JString::from(obj))?.into();
            Ok(out)
        })?;
        let parsed: ExternalCmdJson = serde_json::from_str(&raw)
            .map_err(|e| anyhow::anyhow!("invalid web_fetch JSON: {e}"))?;
        Ok(ExternalToolResult { success: parsed.success, output: parsed.output })
    }

    fn file_read(&self, path: &str) -> Result<ExternalToolResult> {
        let raw = self.with_env(|env| {
            let p = env.new_string(path)?;
            let res = env.call_method(
                self.callback.as_obj(),
                "fileRead",
                "(Ljava/lang/String;)Ljava/lang/String;",
                &[(&p).into()],
            )?;
            let obj = res.l()?;
            let out: String = env.get_string(&JString::from(obj))?.into();
            Ok(out)
        })?;
        let parsed: ExternalCmdJson = serde_json::from_str(&raw)
            .map_err(|e| anyhow::anyhow!("invalid file_read JSON: {e}"))?;
        Ok(ExternalToolResult { success: parsed.success, output: parsed.output })
    }

    fn file_write(&self, path: &str, content: &str, append: bool) -> Result<ExternalToolResult> {
        let raw = self.with_env(|env| {
            let p = env.new_string(path)?;
            let c = env.new_string(content)?;
            let a = if append { 1 } else { 0 };
            let res = env.call_method(
                self.callback.as_obj(),
                "fileWrite",
                "(Ljava/lang/String;Ljava/lang/String;I)Ljava/lang/String;",
                &[(&p).into(), (&c).into(), a.into()],
            )?;
            let obj = res.l()?;
            let out: String = env.get_string(&JString::from(obj))?.into();
            Ok(out)
        })?;
        let parsed: ExternalCmdJson = serde_json::from_str(&raw)
            .map_err(|e| anyhow::anyhow!("invalid file_write JSON: {e}"))?;
        Ok(ExternalToolResult { success: parsed.success, output: parsed.output })
    }
}

fn new_app_handle(app: BuzzClawApp) -> *mut Mutex<BuzzClawApp> {
    Box::into_raw(Box::new(Mutex::new(app)))
}

fn app_from_handle<'a>(handle: jlong) -> Result<&'a Mutex<BuzzClawApp>> {
    if handle == 0 {
        anyhow::bail!("null handle");
    }
    Ok(unsafe { &*(handle as *mut Mutex<BuzzClawApp>) })
}

fn jstring_result(env: &JNIEnv<'_>, result: Result<String>) -> jstring {
    match result {
        Ok(s) => env.new_string(s).map(|j| j.into_raw()).unwrap_or(std::ptr::null_mut()),
        Err(e) => {
            set_last_error(&e.to_string());
            env
                .new_string(format!("error: {e}"))
                .map(|j| j.into_raw())
                .unwrap_or(std::ptr::null_mut())
        }
    }
}

static LAST_ERROR: OnceLock<Mutex<String>> = OnceLock::new();

fn set_last_error(msg: &str) {
    let lock = LAST_ERROR.get_or_init(|| Mutex::new(String::new()));
    if let Ok(mut guard) = lock.lock() {
        *guard = msg.to_string();
    }
}

fn get_last_error() -> String {
    let lock = LAST_ERROR.get_or_init(|| Mutex::new(String::new()));
    lock.lock().map(|g| g.clone()).unwrap_or_default()
}

#[no_mangle]
pub extern "system" fn Java_xyz_buzzster_buzzclaw_BuzzClawNative_init(
    mut env: JNIEnv,
    _class: JClass,
    data_dir: JString,
    callback: JObject,
) -> jlong {
    let result: Result<jlong> = (|| {
        let data_dir: String = env.get_string(&data_dir)?.into();
        config::set_data_dir(data_dir)?;

        let vm = env.get_java_vm()?;
        let cb_global = env.new_global_ref(callback)?;
        let callbacks = Arc::new(JniCallbacks { jvm: vm, callback: cb_global });

        let cb_cmd: Arc<dyn ExternalCmdRunner> = callbacks.clone();
        let cb_tools: Arc<dyn ExternalToolRunner> = callbacks.clone();
        set_external_cmd_runner(cb_cmd)?;
        set_external_tool_runner(cb_tools)?;

        let mut cfg = config::Config::load().unwrap_or_default();
        if cfg.model.trim().is_empty() {
            cfg.model = "local".to_string();
        }

        let app = BuzzClawApp::open_with_config(cfg, AppOptions::default())?;
        Ok(new_app_handle(app) as jlong)
    })();

    match result {
        Ok(h) => h,
        Err(_) => 0,
    }
}

#[no_mangle]
pub extern "system" fn Java_xyz_buzzster_buzzclaw_BuzzClawNative_sendMessage(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    session: JString,
    message: JString,
) -> jstring {
    let result: Result<String> = (|| {
        let session: String = env.get_string(&session)?.into();
        let message: String = env.get_string(&message)?.into();
        let app = app_from_handle(handle)?;
        let guard = app.lock().map_err(|_| anyhow::anyhow!("app lock poisoned"))?;
        guard.send_message(&session, &message)
    })();

    jstring_result(&env, result)
}

#[no_mangle]
pub extern "system" fn Java_xyz_buzzster_buzzclaw_BuzzClawNative_resetSession(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    session: JString,
) -> jboolean {
    let result: Result<bool> = (|| {
        let session: String = env.get_string(&session)?.into();
        let app = app_from_handle(handle)?;
        let guard = app.lock().map_err(|_| anyhow::anyhow!("app lock poisoned"))?;
        guard.reset_session(&session)
    })();

    match result {
        Ok(true) => JNI_TRUE,
        Ok(false) => JNI_FALSE,
        Err(_) => JNI_FALSE,
    }
}

#[no_mangle]
pub extern "system" fn Java_xyz_buzzster_buzzclaw_BuzzClawNative_listSessions(
    env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jstring {
    let result: Result<String> = (|| {
        let app = app_from_handle(handle)?;
        let guard = app.lock().map_err(|_| anyhow::anyhow!("app lock poisoned"))?;
        let list = guard.list_sessions();
        Ok(serde_json::to_string(&list)?)
    })();

    jstring_result(&env, result)
}

#[no_mangle]
pub extern "system" fn Java_xyz_buzzster_buzzclaw_BuzzClawNative_getConfig(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jstring {
    let result: Result<String> = (|| {
        let app = app_from_handle(handle)?;
        let guard = app.lock().map_err(|_| anyhow::anyhow!("app lock poisoned"))?;
        guard.config_json()
    })();
    jstring_result(&env, result)
}

#[no_mangle]
pub extern "system" fn Java_xyz_buzzster_buzzclaw_BuzzClawNative_setConfig(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    json: JString,
) -> jboolean {
    let result: Result<()> = (|| {
        let json: String = env.get_string(&json)?.into();
        let app = app_from_handle(handle)?;
        let mut guard = app.lock().map_err(|_| anyhow::anyhow!("app lock poisoned"))?;
        guard.set_config_json(&json)
    })();
    match result {
        Ok(_) => JNI_TRUE,
        Err(e) => {
            set_last_error(&e.to_string());
            JNI_FALSE
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_xyz_buzzster_buzzclaw_BuzzClawNative_setModel(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    model: JString,
) -> jboolean {
    let result: Result<()> = (|| {
        let model: String = env.get_string(&model)?.into();
        let app = app_from_handle(handle)?;
        let mut guard = app.lock().map_err(|_| anyhow::anyhow!("app lock poisoned"))?;
        guard.set_model(&model)
    })();
    match result {
        Ok(_) => JNI_TRUE,
        Err(e) => {
            set_last_error(&e.to_string());
            JNI_FALSE
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_xyz_buzzster_buzzclaw_BuzzClawNative_setProvider(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    provider: JString,
) -> jboolean {
    let result: Result<()> = (|| {
        let provider: String = env.get_string(&provider)?.into();
        let app = app_from_handle(handle)?;
        let mut guard = app.lock().map_err(|_| anyhow::anyhow!("app lock poisoned"))?;
        guard.set_provider(&provider)
    })();
    match result {
        Ok(_) => JNI_TRUE,
        Err(e) => {
            set_last_error(&e.to_string());
            JNI_FALSE
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_xyz_buzzster_buzzclaw_BuzzClawNative_setApiKey(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    api_key: JString,
) -> jboolean {
    let result: Result<()> = (|| {
        let api_key: String = env.get_string(&api_key)?.into();
        let app = app_from_handle(handle)?;
        let mut guard = app.lock().map_err(|_| anyhow::anyhow!("app lock poisoned"))?;
        let key = if api_key.trim().is_empty() { None } else { Some(api_key.as_str()) };
        guard.set_api_key(key)
    })();
    match result {
        Ok(_) => JNI_TRUE,
        Err(e) => {
            set_last_error(&e.to_string());
            JNI_FALSE
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_xyz_buzzster_buzzclaw_BuzzClawNative_setSystemPrompt(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    prompt: JString,
) -> jboolean {
    let result: Result<()> = (|| {
        let prompt: String = env.get_string(&prompt)?.into();
        let app = app_from_handle(handle)?;
        let mut guard = app.lock().map_err(|_| anyhow::anyhow!("app lock poisoned"))?;
        let p = if prompt.trim().is_empty() { None } else { Some(prompt.as_str()) };
        guard.set_system_prompt(p)
    })();
    match result {
        Ok(_) => JNI_TRUE,
        Err(e) => {
            set_last_error(&e.to_string());
            JNI_FALSE
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_xyz_buzzster_buzzclaw_BuzzClawNative_setTemperature(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    temperature: jni::sys::jdouble,
) -> jboolean {
    let result: Result<()> = (|| {
        let app = app_from_handle(handle)?;
        let mut guard = app.lock().map_err(|_| anyhow::anyhow!("app lock poisoned"))?;
        guard.set_temperature(temperature as f64)
    })();
    match result {
        Ok(_) => JNI_TRUE,
        Err(e) => {
            set_last_error(&e.to_string());
            JNI_FALSE
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_xyz_buzzster_buzzclaw_BuzzClawNative_getLastError(
    env: JNIEnv,
    _class: JClass,
) -> jstring {
    let msg = get_last_error();
    env.new_string(msg).map(|j| j.into_raw()).unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub extern "system" fn Java_xyz_buzzster_buzzclaw_BuzzClawNative_setToolsEnabled(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    enabled: jboolean,
) -> jboolean {
    let result: Result<()> = (|| {
        let app = app_from_handle(handle)?;
        let mut guard = app.lock().map_err(|_| anyhow::anyhow!("app lock poisoned"))?;
        guard.set_tools_enabled(enabled == JNI_TRUE)
    })();
    match result {
        Ok(_) => JNI_TRUE,
        Err(_) => JNI_FALSE,
    }
}

#[no_mangle]
pub extern "system" fn Java_xyz_buzzster_buzzclaw_BuzzClawNative_workspaceList(
    env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jstring {
    let result: Result<String> = (|| {
        let app = app_from_handle(handle)?;
        let guard = app.lock().map_err(|_| anyhow::anyhow!("app lock poisoned"))?;
        guard.workspace_list_json()
    })();
    jstring_result(&env, result)
}

#[no_mangle]
pub extern "system" fn Java_xyz_buzzster_buzzclaw_BuzzClawNative_workspaceRead(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    file: JString,
) -> jstring {
    let result: Result<String> = (|| {
        let file: String = env.get_string(&file)?.into();
        let app = app_from_handle(handle)?;
        let guard = app.lock().map_err(|_| anyhow::anyhow!("app lock poisoned"))?;
        guard.workspace_read(&file)
    })();
    jstring_result(&env, result)
}

#[no_mangle]
pub extern "system" fn Java_xyz_buzzster_buzzclaw_BuzzClawNative_workspaceWrite(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    file: JString,
    content: JString,
    append: jboolean,
) -> jboolean {
    let result: Result<()> = (|| {
        let file: String = env.get_string(&file)?.into();
        let content: String = env.get_string(&content)?.into();
        let app = app_from_handle(handle)?;
        let guard = app.lock().map_err(|_| anyhow::anyhow!("app lock poisoned"))?;
        guard.workspace_write(&file, &content, append == JNI_TRUE)
    })();
    match result {
        Ok(_) => JNI_TRUE,
        Err(_) => JNI_FALSE,
    }
}

#[no_mangle]
pub extern "system" fn Java_xyz_buzzster_buzzclaw_BuzzClawNative_canvasRead(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    name: JString,
) -> jstring {
    let result: Result<String> = (|| {
        let name: String = env.get_string(&name)?.into();
        let app = app_from_handle(handle)?;
        let guard = app.lock().map_err(|_| anyhow::anyhow!("app lock poisoned"))?;
        guard.canvas_read(&name)
    })();
    jstring_result(&env, result)
}

#[no_mangle]
pub extern "system" fn Java_xyz_buzzster_buzzclaw_BuzzClawNative_canvasWrite(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    name: JString,
    content: JString,
    append: jboolean,
) -> jboolean {
    let result: Result<()> = (|| {
        let name: String = env.get_string(&name)?.into();
        let content: String = env.get_string(&content)?.into();
        let app = app_from_handle(handle)?;
        let guard = app.lock().map_err(|_| anyhow::anyhow!("app lock poisoned"))?;
        guard.canvas_write(&name, &content, append == JNI_TRUE)
    })();
    match result {
        Ok(_) => JNI_TRUE,
        Err(_) => JNI_FALSE,
    }
}

#[no_mangle]
pub extern "system" fn Java_xyz_buzzster_buzzclaw_BuzzClawNative_sessionHistory(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    session: JString,
) -> jstring {
    let result: Result<String> = (|| {
        let session: String = env.get_string(&session)?.into();
        let app = app_from_handle(handle)?;
        let guard = app.lock().map_err(|_| anyhow::anyhow!("app lock poisoned"))?;
        guard.session_history_json(&session)
    })();
    jstring_result(&env, result)
}

#[no_mangle]
pub extern "system" fn Java_xyz_buzzster_buzzclaw_BuzzClawNative_setSessionHistory(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    session: JString,
    json: JString,
) -> jboolean {
    let result: Result<()> = (|| {
        let session: String = env.get_string(&session)?.into();
        let json: String = env.get_string(&json)?.into();
        let app = app_from_handle(handle)?;
        let guard = app.lock().map_err(|_| anyhow::anyhow!("app lock poisoned"))?;
        guard.set_session_history_json(&session, &json)
    })();
    match result {
        Ok(_) => JNI_TRUE,
        Err(_) => JNI_FALSE,
    }
}

#[no_mangle]
pub extern "system" fn Java_xyz_buzzster_buzzclaw_BuzzClawNative_destroy(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    if handle == 0 {
        return;
    }
    unsafe {
        drop(Box::from_raw(handle as *mut Mutex<BuzzClawApp>));
    }
}
