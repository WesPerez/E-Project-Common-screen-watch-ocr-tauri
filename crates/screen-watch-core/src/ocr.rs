use crate::{
    build::BuildFlavor,
    data_dir::user_data_dir,
    detect::{OcrTextRow, RgbFrame},
};
use serde::Serialize;
use std::{
    fmt::Debug,
    path::{Path, PathBuf},
};
use thiserror::Error;

pub const OCR_MODEL_DIR_ENV: &str = "SCREENWATCH_OCR_MODEL_DIR";
pub const REQUIRED_NATIVE_OCR_ASSETS: [&str; 3] = ["det.onnx", "rec.onnx", "ppocrv5_dict.txt"];
pub const RAPIDOCR_V6_REFERENCE_MODELS: [&str; 3] = [
    "PP-OCRv6_det_small.onnx",
    "PP-OCRv6_rec_small.onnx",
    "ch_ppocr_mobile_v2.0_cls_mobile.onnx",
];

#[derive(Debug, Clone)]
pub struct OcrSettings {
    pub build_flavor: BuildFlavor,
    pub model_dir: PathBuf,
    pub module_compiled: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OcrAvailability {
    pub enabled: bool,
    pub available: bool,
    pub module_compiled: bool,
    pub models_ready: bool,
    pub backend_ready: bool,
    pub backend_name: String,
    pub model_profile: String,
    pub model_dir: String,
    pub required_models: Vec<OcrModelFileStatus>,
    pub reference_models: Vec<OcrModelFileStatus>,
    pub missing_models: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OcrModelFileStatus {
    pub name: String,
    pub path: String,
    pub exists: bool,
    pub bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OcrProbeResult {
    pub availability: OcrAvailability,
    pub attempted: bool,
    pub initialized: bool,
    pub reason: String,
    pub error: Option<String>,
}

#[derive(Debug, Error)]
pub enum OcrError {
    #[error("{0}")]
    Unavailable(String),
}

pub trait OcrBackend: Debug + Send {
    fn recognize(&mut self, frame: &RgbFrame) -> Result<Vec<OcrTextRow>, OcrError>;
}

#[derive(Debug, Clone)]
pub struct UnavailableOcrBackend {
    reason: String,
}

pub fn create_ocr_backend(settings: &OcrSettings) -> Box<dyn OcrBackend> {
    let availability = settings.availability();
    if !availability.available {
        return Box::new(UnavailableOcrBackend::from_availability(availability));
    }
    create_native_ocr_backend(settings).unwrap_or_else(|err| {
        Box::new(UnavailableOcrBackend::new(format!(
            "OCR backend failed to initialize: {err}"
        )))
    })
}

pub fn probe_ocr_backend(settings: &OcrSettings) -> OcrProbeResult {
    let availability = settings.availability();
    if !availability.available {
        let reason = UnavailableOcrBackend::from_availability(availability.clone())
            .reason()
            .to_string();
        return OcrProbeResult {
            availability,
            attempted: false,
            initialized: false,
            reason,
            error: None,
        };
    }

    match probe_native_ocr_backend(settings) {
        Ok(()) => OcrProbeResult {
            availability,
            attempted: true,
            initialized: true,
            reason: "OCR backend initialized successfully".to_string(),
            error: None,
        },
        Err(err) => OcrProbeResult {
            availability,
            attempted: true,
            initialized: false,
            reason: "OCR backend failed to initialize".to_string(),
            error: Some(err.to_string()),
        },
    }
}

impl OcrSettings {
    pub fn from_env() -> Self {
        Self::from_env_and_data_dir(BuildFlavor::from_env(), user_data_dir())
    }

    pub fn from_env_and_data_dir(build_flavor: BuildFlavor, data_dir: PathBuf) -> Self {
        let model_dir = std::env::var_os(OCR_MODEL_DIR_ENV)
            .map(PathBuf::from)
            .unwrap_or_else(|| default_model_dir(data_dir));
        Self {
            build_flavor,
            model_dir,
            module_compiled: ocr_module_compiled(),
        }
    }

    pub fn from_sources(
        build_flavor: BuildFlavor,
        data_dir: PathBuf,
        env_model_dir: Option<PathBuf>,
    ) -> Self {
        Self::from_sources_with_module(
            build_flavor,
            data_dir,
            env_model_dir,
            build_flavor.ocr_enabled(),
        )
    }

    pub fn from_sources_with_module(
        build_flavor: BuildFlavor,
        data_dir: PathBuf,
        env_model_dir: Option<PathBuf>,
        module_compiled: bool,
    ) -> Self {
        Self {
            build_flavor,
            model_dir: env_model_dir.unwrap_or_else(|| default_model_dir(data_dir)),
            module_compiled,
        }
    }

    pub fn availability(&self) -> OcrAvailability {
        let required_models = required_model_statuses(&self.model_dir);
        let reference_models = rapidocr_reference_model_statuses(&self.model_dir);
        let backend_name = native_ocr_backend_name().to_string();
        let model_profile = native_ocr_model_profile().to_string();
        if !self.build_flavor.ocr_enabled() {
            return OcrAvailability {
                enabled: false,
                available: false,
                module_compiled: self.module_compiled,
                models_ready: false,
                backend_ready: false,
                backend_name,
                model_profile,
                model_dir: self.model_dir.display().to_string(),
                required_models,
                reference_models,
                missing_models: Vec::new(),
                reason: "lite build: OCR module disabled".to_string(),
            };
        }
        if !self.module_compiled {
            return OcrAvailability {
                enabled: true,
                available: false,
                module_compiled: false,
                models_ready: false,
                backend_ready: false,
                backend_name,
                model_profile,
                model_dir: self.model_dir.display().to_string(),
                required_models,
                reference_models,
                missing_models: Vec::new(),
                reason:
                    "full build requested, but OCR module was not compiled into this executable"
                        .to_string(),
            };
        }

        let missing_models = required_models
            .iter()
            .filter(|model| !model.exists)
            .map(|model| model.name.clone())
            .collect::<Vec<_>>();
        let models_ready = missing_models.is_empty();
        let backend_ready = native_ocr_backend_linked();
        let available = models_ready && backend_ready;
        OcrAvailability {
            enabled: true,
            available,
            module_compiled: true,
            models_ready,
            backend_ready,
            backend_name,
            model_profile,
            model_dir: self.model_dir.display().to_string(),
            required_models,
            reference_models,
            reason: if !models_ready {
                "full build: native OCR assets missing from external model directory".to_string()
            } else if !backend_ready {
                "OCR native assets are ready, but the native OCR inference backend is not linked yet"
                    .to_string()
            } else {
                "OCR ready; model integrity is verified when the backend is first initialized"
                    .to_string()
            },
            missing_models,
        }
    }
}

pub fn ocr_module_compiled() -> bool {
    cfg!(feature = "ocr")
}

pub fn native_ocr_backend_linked() -> bool {
    cfg!(feature = "ocr")
}

pub fn native_ocr_backend_name() -> &'static str {
    if native_ocr_backend_linked() {
        "pure-onnx-ocr"
    } else {
        "not-linked"
    }
}

pub fn native_ocr_model_profile() -> &'static str {
    "ppocrv5-dbnet-svtr"
}

impl UnavailableOcrBackend {
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }

    pub fn disabled() -> Self {
        Self {
            reason: "OCR backend disabled for this scan engine".to_string(),
        }
    }

    pub fn from_settings(settings: &OcrSettings) -> Self {
        Self::from_availability(settings.availability())
    }

    pub fn from_availability(availability: OcrAvailability) -> Self {
        let reason = if !availability.enabled {
            availability.reason
        } else if !availability.module_compiled {
            availability.reason
        } else if !availability.models_ready {
            if availability.missing_models.is_empty() {
                availability.reason
            } else {
                format!(
                    "{}: {}",
                    availability.reason,
                    availability.missing_models.join(", ")
                )
            }
        } else if !availability.backend_ready {
            availability.reason
        } else if !availability.available {
            availability.reason
        } else {
            "OCR backend was requested through the unavailable backend path".to_string()
        };
        Self { reason }
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }
}

impl OcrBackend for UnavailableOcrBackend {
    fn recognize(&mut self, _frame: &RgbFrame) -> Result<Vec<OcrTextRow>, OcrError> {
        Err(OcrError::Unavailable(self.reason.clone()))
    }
}

pub fn default_model_dir(data_dir: PathBuf) -> PathBuf {
    data_dir.join("models").join("rapidocr")
}

pub fn required_model_statuses(model_dir: &Path) -> Vec<OcrModelFileStatus> {
    REQUIRED_NATIVE_OCR_ASSETS
        .iter()
        .map(|name| model_file_status(model_dir, name))
        .collect()
}

pub fn rapidocr_reference_model_statuses(model_dir: &Path) -> Vec<OcrModelFileStatus> {
    RAPIDOCR_V6_REFERENCE_MODELS
        .iter()
        .map(|name| model_file_status(model_dir, name))
        .collect()
}

pub fn missing_models(model_dir: &Path) -> Vec<String> {
    required_model_statuses(model_dir)
        .into_iter()
        .filter(|model| !model.exists)
        .map(|model| model.name)
        .collect()
}

fn model_file_status(model_dir: &Path, name: &str) -> OcrModelFileStatus {
    let path = model_dir.join(name);
    let metadata = path.metadata().ok().filter(|meta| meta.is_file());
    OcrModelFileStatus {
        name: name.to_string(),
        path: path.display().to_string(),
        exists: metadata.is_some(),
        bytes: metadata.map(|meta| meta.len()),
    }
}

#[cfg(feature = "ocr")]
struct NativeOcrBackend {
    model_dir: PathBuf,
    worker: Option<NativeOcrWorker>,
}

#[cfg(feature = "ocr")]
impl NativeOcrBackend {
    fn from_model_dir(model_dir: &Path) -> Self {
        Self {
            model_dir: model_dir.to_path_buf(),
            worker: None,
        }
    }

    fn worker(&mut self) -> Result<&mut NativeOcrWorker, OcrError> {
        if self.worker.is_none() {
            self.worker = Some(NativeOcrWorker::spawn(self.model_dir.clone())?);
        }
        self.worker
            .as_mut()
            .ok_or_else(|| OcrError::Unavailable("OCR worker is not running".to_string()))
    }
}

#[cfg(feature = "ocr")]
impl Debug for NativeOcrBackend {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NativeOcrBackend")
            .field("model_dir", &self.model_dir)
            .field("worker_running", &self.worker.is_some())
            .finish()
    }
}

#[cfg(feature = "ocr")]
impl OcrBackend for NativeOcrBackend {
    fn recognize(&mut self, frame: &RgbFrame) -> Result<Vec<OcrTextRow>, OcrError> {
        self.worker()?.recognize(frame.clone())
    }
}

#[cfg(feature = "ocr")]
#[derive(Debug)]
struct NativeOcrWorker {
    sender: std::sync::mpsc::Sender<NativeOcrMessage>,
    handle: Option<std::thread::JoinHandle<()>>,
}

#[cfg(feature = "ocr")]
enum NativeOcrMessage {
    Recognize {
        frame: RgbFrame,
        respond_to: std::sync::mpsc::Sender<Result<Vec<OcrTextRow>, String>>,
    },
    Shutdown,
}

#[cfg(feature = "ocr")]
enum NativeOcrEngineState {
    Uninitialized,
    Ready(Box<pure_onnx_ocr::OcrEngine>),
    Failed(String),
}

#[cfg(feature = "ocr")]
impl NativeOcrWorker {
    fn spawn(model_dir: PathBuf) -> Result<Self, OcrError> {
        let (sender, receiver) = std::sync::mpsc::channel();
        let handle = std::thread::Builder::new()
            .name("screen-watch-ocr-native-ocr".to_string())
            .spawn(move || native_ocr_worker_loop(model_dir, receiver))
            .map_err(|err| {
                OcrError::Unavailable(format!("failed to start OCR worker thread: {err}"))
            })?;
        Ok(Self {
            sender,
            handle: Some(handle),
        })
    }

    fn recognize(&mut self, frame: RgbFrame) -> Result<Vec<OcrTextRow>, OcrError> {
        let (respond_to, response) = std::sync::mpsc::channel();
        self.sender
            .send(NativeOcrMessage::Recognize { frame, respond_to })
            .map_err(|_| OcrError::Unavailable("OCR worker is not running".to_string()))?;
        response
            .recv()
            .map_err(|_| OcrError::Unavailable("OCR worker stopped before replying".to_string()))?
            .map_err(OcrError::Unavailable)
    }
}

#[cfg(feature = "ocr")]
impl Drop for NativeOcrWorker {
    fn drop(&mut self) {
        let _ = self.sender.send(NativeOcrMessage::Shutdown);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(feature = "ocr")]
fn native_ocr_worker_loop(
    model_dir: PathBuf,
    receiver: std::sync::mpsc::Receiver<NativeOcrMessage>,
) {
    let mut state = NativeOcrEngineState::Uninitialized;
    while let Ok(message) = receiver.recv() {
        match message {
            NativeOcrMessage::Recognize { frame, respond_to } => {
                let result = native_ocr_worker_recognize(&mut state, &model_dir, frame)
                    .map_err(|err| err.to_string());
                let _ = respond_to.send(result);
            }
            NativeOcrMessage::Shutdown => break,
        }
    }
}

#[cfg(feature = "ocr")]
fn native_ocr_worker_recognize(
    state: &mut NativeOcrEngineState,
    model_dir: &Path,
    frame: RgbFrame,
) -> Result<Vec<OcrTextRow>, OcrError> {
    match state {
        NativeOcrEngineState::Uninitialized => match build_native_ocr_engine(model_dir) {
            Ok(engine) => {
                *state = NativeOcrEngineState::Ready(Box::new(engine));
            }
            Err(err) => {
                let message = format!("OCR backend failed to initialize: {err}");
                *state = NativeOcrEngineState::Failed(message.clone());
                return Err(OcrError::Unavailable(message));
            }
        },
        NativeOcrEngineState::Ready(_) | NativeOcrEngineState::Failed(_) => {}
    }

    match state {
        NativeOcrEngineState::Ready(engine) => recognize_with_native_engine(engine, &frame),
        NativeOcrEngineState::Failed(message) => Err(OcrError::Unavailable(message.clone())),
        NativeOcrEngineState::Uninitialized => {
            unreachable!("OCR engine state is initialized above")
        }
    }
}

#[cfg(feature = "ocr")]
fn build_native_ocr_engine(model_dir: &Path) -> Result<pure_onnx_ocr::OcrEngine, OcrError> {
    pure_onnx_ocr::OcrEngineBuilder::new()
        .det_model_path(model_dir.join(REQUIRED_NATIVE_OCR_ASSETS[0]))
        .rec_model_path(model_dir.join(REQUIRED_NATIVE_OCR_ASSETS[1]))
        .dictionary_path(model_dir.join(REQUIRED_NATIVE_OCR_ASSETS[2]))
        .build()
        .map_err(|err| OcrError::Unavailable(err.to_string()))
}

#[cfg(feature = "ocr")]
fn recognize_with_native_engine(
    engine: &pure_onnx_ocr::OcrEngine,
    frame: &RgbFrame,
) -> Result<Vec<OcrTextRow>, OcrError> {
    let image = rgb_frame_to_dynamic_image(frame)?;
    let rows = engine
        .run_from_image(&image)
        .map_err(|err| OcrError::Unavailable(err.to_string()))?;
    Ok(rows
        .into_iter()
        .map(|row| OcrTextRow {
            text: row.text,
            score: row.confidence,
            box_points: Some(
                row.bounding_box
                    .exterior()
                    .points()
                    .map(|point| [point.x() as f32, point.y() as f32])
                    .collect(),
            ),
        })
        .collect())
}

#[cfg(feature = "ocr")]
fn rgb_frame_to_dynamic_image(frame: &RgbFrame) -> Result<image::DynamicImage, OcrError> {
    let image = image::RgbImage::from_raw(frame.width, frame.height, frame.pixels.clone())
        .ok_or_else(|| {
            OcrError::Unavailable("OCR frame has invalid RGB buffer size".to_string())
        })?;
    Ok(image::DynamicImage::ImageRgb8(image))
}

#[cfg(feature = "ocr")]
fn create_native_ocr_backend(settings: &OcrSettings) -> Result<Box<dyn OcrBackend>, OcrError> {
    Ok(Box::new(NativeOcrBackend::from_model_dir(
        &settings.model_dir,
    )))
}

#[cfg(feature = "ocr")]
fn probe_native_ocr_backend(settings: &OcrSettings) -> Result<(), OcrError> {
    build_native_ocr_engine(&settings.model_dir).map(|_| ())
}

#[cfg(not(feature = "ocr"))]
fn create_native_ocr_backend(_settings: &OcrSettings) -> Result<Box<dyn OcrBackend>, OcrError> {
    Err(OcrError::Unavailable(
        "native OCR inference backend is not linked".to_string(),
    ))
}

#[cfg(not(feature = "ocr"))]
fn probe_native_ocr_backend(_settings: &OcrSettings) -> Result<(), OcrError> {
    Err(OcrError::Unavailable(
        "native OCR inference backend is not linked".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        create_ocr_backend, default_model_dir, native_ocr_backend_linked, ocr_module_compiled,
        probe_ocr_backend, rapidocr_reference_model_statuses, required_model_statuses, OcrSettings,
        RAPIDOCR_V6_REFERENCE_MODELS, REQUIRED_NATIVE_OCR_ASSETS,
    };
    use crate::build::BuildFlavor;
    use crate::detect::RgbFrame;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn ocr_models_default_to_shared_app_data_dir() {
        let data_dir = PathBuf::from(r"C:\Users\Wes\AppData\Local\ScreenWatchOCR");
        assert_eq!(
            default_model_dir(data_dir),
            PathBuf::from(r"C:\Users\Wes\AppData\Local\ScreenWatchOCR\models\rapidocr")
        );
    }

    #[test]
    fn lite_build_reports_ocr_disabled_without_requiring_models() {
        let settings = OcrSettings::from_sources(
            BuildFlavor::Lite,
            PathBuf::from("app-data"),
            Some(PathBuf::from("missing")),
        );
        let availability = settings.availability();
        assert!(!availability.enabled);
        assert!(!availability.available);
        assert!(!availability.models_ready);
        assert!(!availability.backend_ready);
        assert!(availability.missing_models.is_empty());
    }

    #[test]
    fn full_build_reports_missing_external_models() {
        let tmp = tempfile::tempdir().unwrap();
        let settings = OcrSettings::from_sources(BuildFlavor::Full, tmp.path().to_path_buf(), None);
        let availability = settings.availability();
        assert!(availability.enabled);
        assert!(!availability.available);
        assert!(availability.module_compiled);
        assert!(!availability.models_ready);
        assert_eq!(availability.missing_models, REQUIRED_NATIVE_OCR_ASSETS);
        assert_eq!(availability.model_profile, "ppocrv5-dbnet-svtr");
    }

    #[test]
    fn full_flavor_reports_when_ocr_module_is_not_compiled() {
        let tmp = tempfile::tempdir().unwrap();
        let settings = OcrSettings::from_sources_with_module(
            BuildFlavor::Full,
            tmp.path().to_path_buf(),
            None,
            false,
        );
        let availability = settings.availability();
        assert!(availability.enabled);
        assert!(!availability.available);
        assert!(!availability.module_compiled);
        assert!(!availability.models_ready);
        assert!(availability.missing_models.is_empty());
        assert!(availability.reason.contains("not compiled"));
    }

    #[test]
    fn full_build_accepts_external_models_but_waits_for_native_backend() {
        let tmp = tempfile::tempdir().unwrap();
        let model_dir = tmp.path().join("models");
        fs::create_dir_all(&model_dir).unwrap();
        for name in REQUIRED_NATIVE_OCR_ASSETS {
            fs::write(model_dir.join(name), b"placeholder").unwrap();
        }

        let settings =
            OcrSettings::from_sources(BuildFlavor::Full, tmp.path().to_path_buf(), Some(model_dir));
        let availability = settings.availability();
        assert_eq!(availability.available, cfg!(feature = "ocr"));
        assert!(availability.models_ready);
        assert_eq!(availability.backend_ready, cfg!(feature = "ocr"));
        assert!(availability.missing_models.is_empty());
        if cfg!(feature = "ocr") {
            assert!(availability.reason.contains("model integrity"));
            assert_eq!(availability.backend_name, "pure-onnx-ocr");
        } else {
            assert!(availability.reason.contains("native OCR inference backend"));
            assert_eq!(availability.backend_name, "not-linked");
        }
        assert_eq!(
            availability
                .required_models
                .iter()
                .map(|model| (model.name.as_str(), model.exists))
                .collect::<Vec<_>>(),
            REQUIRED_NATIVE_OCR_ASSETS
                .iter()
                .map(|name| (*name, true))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn created_backend_reports_lite_disabled_state() {
        let settings = OcrSettings::from_sources(
            BuildFlavor::Lite,
            PathBuf::from("app-data"),
            Some(PathBuf::from("unused")),
        );
        let mut backend = create_ocr_backend(&settings);
        let frame = empty_frame();

        let err = backend.recognize(&frame).unwrap_err();

        assert!(err.to_string().contains("OCR module disabled"));
    }

    #[test]
    fn created_backend_reports_missing_model_names() {
        let tmp = tempfile::tempdir().unwrap();
        let settings = OcrSettings::from_sources(BuildFlavor::Full, tmp.path().to_path_buf(), None);
        let mut backend = create_ocr_backend(&settings);
        let frame = empty_frame();

        let err = backend.recognize(&frame).unwrap_err().to_string();

        assert!(err.contains("OCR assets missing"));
        assert!(err.contains(REQUIRED_NATIVE_OCR_ASSETS[0]));
    }

    #[test]
    fn created_backend_reports_link_or_model_load_state_after_models_ready() {
        let tmp = tempfile::tempdir().unwrap();
        let model_dir = tmp.path().join("models");
        fs::create_dir_all(&model_dir).unwrap();
        for name in REQUIRED_NATIVE_OCR_ASSETS {
            fs::write(model_dir.join(name), b"placeholder").unwrap();
        }
        let settings =
            OcrSettings::from_sources(BuildFlavor::Full, tmp.path().to_path_buf(), Some(model_dir));
        let mut backend = create_ocr_backend(&settings);
        let frame = empty_frame();

        let err = backend.recognize(&frame).unwrap_err();

        if cfg!(feature = "ocr") {
            assert!(err.to_string().contains("failed to initialize"));
        } else {
            assert!(err.to_string().contains("native OCR inference backend"));
        }
    }

    #[test]
    fn ocr_probe_skips_lite_build_without_initializing_backend() {
        let settings = OcrSettings::from_sources(
            BuildFlavor::Lite,
            PathBuf::from("app-data"),
            Some(PathBuf::from("unused")),
        );

        let probe = probe_ocr_backend(&settings);

        assert!(!probe.availability.enabled);
        assert!(!probe.attempted);
        assert!(!probe.initialized);
        assert!(probe.reason.contains("OCR module disabled"));
        assert!(probe.error.is_none());
    }

    #[test]
    fn ocr_probe_skips_full_build_when_models_are_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let settings = OcrSettings::from_sources(BuildFlavor::Full, tmp.path().to_path_buf(), None);

        let probe = probe_ocr_backend(&settings);

        assert!(probe.availability.enabled);
        assert!(!probe.attempted);
        assert!(!probe.initialized);
        assert!(probe.reason.contains("OCR assets missing"));
        assert!(probe.reason.contains(REQUIRED_NATIVE_OCR_ASSETS[0]));
        assert!(probe.error.is_none());
    }

    #[test]
    fn ocr_probe_reports_not_compiled_full_runtime_without_model_checks() {
        let tmp = tempfile::tempdir().unwrap();
        let settings = OcrSettings::from_sources_with_module(
            BuildFlavor::Full,
            tmp.path().to_path_buf(),
            None,
            false,
        );

        let probe = probe_ocr_backend(&settings);

        assert!(!probe.availability.module_compiled);
        assert!(!probe.attempted);
        assert!(!probe.initialized);
        assert!(probe.reason.contains("not compiled"));
        assert!(probe.error.is_none());
    }

    #[test]
    fn ocr_probe_reports_link_or_model_initialization_state_after_models_ready() {
        let tmp = tempfile::tempdir().unwrap();
        let model_dir = tmp.path().join("models");
        fs::create_dir_all(&model_dir).unwrap();
        for name in REQUIRED_NATIVE_OCR_ASSETS {
            fs::write(model_dir.join(name), b"placeholder").unwrap();
        }
        let settings =
            OcrSettings::from_sources(BuildFlavor::Full, tmp.path().to_path_buf(), Some(model_dir));

        let probe = probe_ocr_backend(&settings);

        assert!(probe.availability.models_ready);
        if cfg!(feature = "ocr") {
            assert!(probe.attempted);
            assert!(!probe.initialized);
            assert!(probe.reason.contains("failed to initialize"));
            assert!(probe.error.as_deref().unwrap_or_default().len() > 0);
        } else {
            assert!(!probe.attempted);
            assert!(!probe.initialized);
            assert!(probe.reason.contains("native OCR inference backend"));
            assert!(probe.error.is_none());
        }
    }

    #[cfg(feature = "ocr")]
    #[test]
    fn native_ocr_worker_reuses_cached_initialization_failure() {
        let tmp = tempfile::tempdir().unwrap();
        let model_dir = tmp.path().join("models");
        fs::create_dir_all(&model_dir).unwrap();
        for name in REQUIRED_NATIVE_OCR_ASSETS {
            fs::write(model_dir.join(name), b"placeholder").unwrap();
        }
        let settings =
            OcrSettings::from_sources(BuildFlavor::Full, tmp.path().to_path_buf(), Some(model_dir));
        let mut backend = create_ocr_backend(&settings);
        let frame = empty_frame();

        let first = backend.recognize(&frame).unwrap_err().to_string();
        fs::remove_file(settings.model_dir.join(REQUIRED_NATIVE_OCR_ASSETS[0])).unwrap();
        let second = backend.recognize(&frame).unwrap_err().to_string();

        assert_eq!(first, second);
        assert!(second.contains("failed to initialize"));
    }

    #[test]
    fn ocr_module_compiled_tracks_cargo_feature() {
        assert_eq!(ocr_module_compiled(), cfg!(feature = "ocr"));
    }

    #[test]
    fn native_ocr_backend_link_state_tracks_cargo_feature() {
        assert_eq!(native_ocr_backend_linked(), cfg!(feature = "ocr"));
    }

    #[test]
    fn required_model_status_reports_paths_and_sizes() {
        let tmp = tempfile::tempdir().unwrap();
        let existing = tmp.path().join(REQUIRED_NATIVE_OCR_ASSETS[0]);
        fs::write(&existing, b"model").unwrap();

        let statuses = required_model_statuses(tmp.path());

        assert_eq!(statuses.len(), REQUIRED_NATIVE_OCR_ASSETS.len());
        assert_eq!(statuses[0].name, REQUIRED_NATIVE_OCR_ASSETS[0]);
        assert_eq!(statuses[0].bytes, Some(5));
        assert!(statuses[0].exists);
        assert!(statuses[0].path.ends_with(REQUIRED_NATIVE_OCR_ASSETS[0]));
        assert!(!statuses[1].exists);
        assert_eq!(statuses[1].bytes, None);
    }

    #[test]
    fn rapidocr_v6_reference_status_is_reported_separately() {
        let tmp = tempfile::tempdir().unwrap();
        let existing = tmp.path().join(RAPIDOCR_V6_REFERENCE_MODELS[0]);
        fs::write(&existing, b"model").unwrap();

        let statuses = rapidocr_reference_model_statuses(tmp.path());

        assert_eq!(statuses.len(), RAPIDOCR_V6_REFERENCE_MODELS.len());
        assert_eq!(statuses[0].name, RAPIDOCR_V6_REFERENCE_MODELS[0]);
        assert!(statuses[0].exists);
        assert!(!statuses[1].exists);
    }

    #[test]
    fn rapidocr_v6_reference_models_do_not_satisfy_native_ppocrv5_profile() {
        let tmp = tempfile::tempdir().unwrap();
        let model_dir = tmp.path().join("models");
        fs::create_dir_all(&model_dir).unwrap();
        for name in RAPIDOCR_V6_REFERENCE_MODELS {
            fs::write(model_dir.join(name), b"model").unwrap();
        }

        let settings =
            OcrSettings::from_sources(BuildFlavor::Full, tmp.path().to_path_buf(), Some(model_dir));
        let availability = settings.availability();

        assert!(!availability.models_ready);
        assert!(!availability.available);
        assert_eq!(availability.missing_models, REQUIRED_NATIVE_OCR_ASSETS);
        assert!(availability
            .reference_models
            .iter()
            .all(|model| model.exists));
        assert_eq!(availability.model_profile, "ppocrv5-dbnet-svtr");
        assert!(availability.reason.contains("native OCR assets missing"));
    }

    #[cfg(feature = "ocr")]
    #[test]
    #[ignore = "requires real external OCR model assets"]
    fn native_ocr_real_model_probe_initializes_from_external_assets() {
        let settings =
            OcrSettings::from_env_and_data_dir(BuildFlavor::Full, crate::data_dir::user_data_dir());
        let availability = settings.availability();

        assert!(
            availability.models_ready,
            "missing external OCR assets in {}: {:?}",
            settings.model_dir.display(),
            availability.missing_models
        );
        assert!(
            availability.backend_ready,
            "native OCR backend is not linked: {}",
            availability.backend_name
        );

        let probe = probe_ocr_backend(&settings);

        assert!(probe.attempted, "OCR probe did not attempt initialization");
        assert!(
            probe.initialized,
            "OCR backend probe failed for {}: {}",
            settings.model_dir.display(),
            probe.error.clone().unwrap_or(probe.reason)
        );
    }

    #[cfg(feature = "ocr")]
    #[test]
    #[ignore = "requires real external OCR model assets and SCREENWATCH_OCR_SMOKE_IMAGE"]
    fn native_ocr_real_model_recognizes_smoke_png() {
        let image_path = std::env::var_os("SCREENWATCH_OCR_SMOKE_IMAGE")
            .map(PathBuf::from)
            .expect("SCREENWATCH_OCR_SMOKE_IMAGE must point to a PNG smoke image");
        let expected = std::env::var("SCREENWATCH_OCR_SMOKE_EXPECT")
            .expect("SCREENWATCH_OCR_SMOKE_EXPECT must contain expected recognized text");
        let frame = RgbFrame::from_png_path(&image_path)
            .unwrap_or_else(|err| panic!("failed to load OCR smoke PNG {image_path:?}: {err}"));
        let settings =
            OcrSettings::from_env_and_data_dir(BuildFlavor::Full, crate::data_dir::user_data_dir());
        let mut backend = create_ocr_backend(&settings);

        let rows = backend
            .recognize(&frame)
            .unwrap_or_else(|err| panic!("OCR recognition failed for {image_path:?}: {err}"));
        let combined = rows
            .iter()
            .map(|row| row.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            !rows.is_empty(),
            "OCR smoke image produced no text rows from {image_path:?}"
        );
        assert!(
            combined.to_lowercase().contains(&expected.to_lowercase()),
            "OCR smoke text did not contain {expected:?}; rows were: {combined:?}"
        );
    }

    fn empty_frame() -> RgbFrame {
        RgbFrame::new(1, 1, vec![0, 0, 0]).unwrap()
    }
}
