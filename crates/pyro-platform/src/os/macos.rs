//! macOS: recepción del evento Apple `kAEOpenDocuments`.
//!
//! winit no expone `application:openURLs:` en su delegate, así que se
//! registra un manejador propio en `NSAppleEventManager`. Tiene que
//! instalarse en `applicationWillFinishLaunching`: si se hace antes,
//! `NSApplication` lo sobrescribe con el suyo al terminar de arrancar, y si se
//! hace después se pierden los archivos con los que se lanzó la app. Como el
//! delegate es de winit, se escucha la notificación equivalente
//! `NSApplicationWillFinishLaunchingNotification`, que no depende de él.

use std::path::PathBuf;
use std::ptr::NonNull;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::NSObject;
use objc2::{define_class, msg_send, sel, AllocAnyThread};
use objc2_foundation::{NSAppleEventDescriptor, NSAppleEventManager, NSNotification, NSNotificationCenter, NSString};

/// Códigos de cuatro caracteres de Apple Events.
const K_CORE_EVENT_CLASS: u32 = u32::from_be_bytes(*b"aevt");
const K_AE_OPEN_DOCUMENTS: u32 = u32::from_be_bytes(*b"odoc");
const KEY_DIRECT_OBJECT: u32 = u32::from_be_bytes(*b"----");

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "PyroOpenDocumentsHandler"]
    struct OpenDocumentsHandler;

    impl OpenDocumentsHandler {
        #[unsafe(method(handleOpenDocuments:withReplyEvent:))]
        fn handle_open_documents(&self, event: &NSAppleEventDescriptor, _reply: &NSAppleEventDescriptor) {
            crate::open_files::queue(file_paths(event));
        }
    }
);

impl OpenDocumentsHandler {
    fn new() -> Retained<Self> {
        let this = Self::alloc().set_ivars(());
        unsafe { msg_send![super(this), init] }
    }
}

/// Extrae las rutas de la lista de archivos del evento (objeto directo).
fn file_paths(event: &NSAppleEventDescriptor) -> Vec<PathBuf> {
    let Some(list) = event.paramDescriptorForKeyword(KEY_DIRECT_OBJECT) else {
        return Vec::new();
    };
    // Los índices de las listas de Apple Events empiezan en 1.
    (1..=list.numberOfItems())
        .filter_map(|i| list.descriptorAtIndex(i))
        .filter_map(|item| item.fileURLValue())
        .filter_map(|url| url.path())
        .map(|path| PathBuf::from(path.to_string()))
        .collect()
}

pub fn install_open_documents_handler() {
    let block = RcBlock::new(|_: NonNull<NSNotification>| {
        let handler = OpenDocumentsHandler::new();
        unsafe {
            NSAppleEventManager::sharedAppleEventManager().setEventHandler_andSelector_forEventClass_andEventID(
                &handler,
                sel!(handleOpenDocuments:withReplyEvent:),
                K_CORE_EVENT_CLASS,
                K_AE_OPEN_DOCUMENTS,
            );
        }
        // El gestor de eventos no retiene el manejador: debe vivir siempre.
        std::mem::forget(handler);
    });
    let name = NSString::from_str("NSApplicationWillFinishLaunchingNotification");
    let observer = unsafe {
        NSNotificationCenter::defaultCenter().addObserverForName_object_queue_usingBlock(
            Some(&name),
            None,
            None,
            &block,
        )
    };
    // La observación dura toda la vida del proceso.
    std::mem::forget(observer);
}
