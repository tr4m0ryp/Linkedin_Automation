//! CSS selectors for LinkedIn profile page elements.
//!
//! LinkedIn frequently changes class names so we keep multiple fallback
//! selectors per logical element. Ordered from most specific to least.

/// Connect button on the profile header.
pub const CONNECT_BUTTON: &[&str] = &[
    "button[aria-label*='Invite'][aria-label*='connect']",
    "button[aria-label*='Connect']",
    "button.pvs-profile-actions__action[aria-label*='Connect']",
    "main button[aria-label*='connect' i]",
];

/// "More" / three-dot dropdown button (if Connect is hidden).
pub const MORE_BUTTON: &[&str] = &[
    "button[aria-label='More actions']",
    "button[aria-label='More']",
    "div.pvs-profile-actions button.artdeco-dropdown__trigger",
];

/// Connect option inside the "More" dropdown.
pub const DROPDOWN_CONNECT: &[&str] = &[
    "div[aria-label*='connect' i]",
    "span.display-flex > span[aria-hidden='true']",
    "li.artdeco-dropdown__item[aria-label*='Connect']",
];

/// "Send without a note" button inside the Add-a-note modal.
pub const SEND_WITHOUT_NOTE: &[&str] = &[
    "button[aria-label='Send without a note']",
    "button[aria-label='Send now']",
    "button.artdeco-button--secondary",
];

/// "Dismiss" button to close a modal.
pub const MODAL_DISMISS: &[&str] = &[
    "button[aria-label='Dismiss']",
    "button[aria-label='Got it']",
    "button.artdeco-modal__dismiss",
];

/// Indicators that a connection request is already pending.
pub const PENDING_INDICATORS: &[&str] = &[
    "button[aria-label*='Pending']",
    "span.artdeco-button__text:has-text('Pending')",
    "button[disabled][aria-label*='Pending']",
];

/// Indicators that we are already connected (Message button present).
pub const ALREADY_CONNECTED: &[&str] = &[
    "button[aria-label*='Message']",
    "a[aria-label*='Message']",
];
