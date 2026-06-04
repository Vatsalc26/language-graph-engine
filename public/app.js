// Global state cache
let activeSnapshot = null;
let currentSymbolList = [];

// Init on load
document.addEventListener("DOMContentLoaded", () => {
    fetchStatus();
    fetchCollections();
    fetchSymbols();
    fetchActiveSnapshot();
    fetchStoreSummary();
    fetchStoreWords();
});


// Clipboard helper
function copyText(text) {
    navigator.clipboard.writeText(text).then(() => {
        alert("Copied to clipboard: " + text);
    }).catch(err => {
        console.error("Failed to copy: ", err);
    });
}

// Fetch status summary
async function fetchStatus() {
    try {
        const res = await fetch("/api/status");
        if (!res.ok) throw new Error(await res.text());
        const data = await res.json();
        
        document.getElementById("snapshot-cid").innerText = data.activeSnapshotCid;
        document.getElementById("identifier-format").innerText = data.identifierFormat;
        document.getElementById("symbol-count").innerText = data.symbolCount;
    } catch (e) {
        showError("Failed to fetch engine status: " + e.message);
    }
}

// Fetch active profile collections
async function fetchCollections() {
    try {
        const res = await fetch("/api/collections");
        if (!res.ok) throw new Error(await res.text());
        const collections = await res.json();

        const tbody = document.getElementById("collections-registry-body");
        tbody.innerHTML = "";

        collections.forEach(col => {
            const tr = document.createElement("tr");
            tr.innerHTML = `
                <td style="font-weight: 500; color: #fff;">${col.label}</td>
                <td><strong>${col.symbolCount}</strong></td>
                <td><span class="cid-badge" onclick="copyText('${col.snapshotCid}')">${col.snapshotCid}</span></td>
                <td><span class="badge badge-healthy">${col.status}</span></td>
            `;
            tbody.appendChild(tr);
        });
    } catch (e) {
        showError("Failed to load collections: " + e.message);
    }
}

// Helper to visibly render special chars
function getDisplayCharLocal(surfaceForm) {
    if (surfaceForm === " ") return "␠ SPACE";
    if (surfaceForm === "\\") return "\\ REVERSE SOLIDUS";
    if (surfaceForm === "`") return "` GRAVE ACCENT";
    return surfaceForm;
}

// Fetch all active symbols
async function fetchSymbols() {
    try {
        const res = await fetch("/api/symbols");
        if (!res.ok) throw new Error(await res.text());
        const symbols = await res.json();
        currentSymbolList = symbols;

        // Render collection/profile label on status block
        if (symbols.length === 95) {
            document.getElementById("collection-name").innerText = "Printable ASCII Text Profile";
        } else if (symbols.length === 74) {
            document.getElementById("collection-name").innerText = "Basic English Written Text Profile";
        } else if (symbols.length === 26) {
            document.getElementById("collection-name").innerText = "Latin lowercase alphabet a-z";
        } else {
            document.getElementById("collection-name").innerText = "Custom Profile";
        }

        const tbody = document.getElementById("symbol-registry-body");
        tbody.innerHTML = "";

        symbols.forEach(sym => {
            const tr = document.createElement("tr");
            tr.className = "clickable-row";
            tr.onclick = () => showSymbolDetails(sym.canonicalEntityId);
            
            const displayChar = getDisplayCharLocal(sym.surfaceForm);
            const collectionLabel = sym.sourceCollectionEntityId.split(":").pop();

            tr.innerHTML = `
                <td style="font-size: 1.15rem; font-weight: 600; color: #a5b4fc;">${displayChar}</td>
                <td><span style="color: #fff;">${getDisplayNameLocal(sym.surfaceForm)}</span></td>
                <td><span class="badge badge-reused">${sym.category}</span></td>
                <td><span style="font-size: 0.85rem; color: var(--text-secondary);">${collectionLabel}</span></td>
                <td><span style="font-family: monospace; font-size: 0.85rem;">${sym.canonicalEntityId}</span></td>
                <td><span class="cid-badge" style="font-size: 0.8rem;" onclick="event.stopPropagation(); copyText('${sym.activeRevisionCid}')">${sym.activeRevisionCid.substring(0, 8)}...</span></td>
                <td><span class="badge badge-healthy">Healthy</span></td>
            `;
            tbody.appendChild(tr);
        });
    } catch (e) {
        showError("Failed to load symbols registry: " + e.message);
    }
}

// Simple display name generator helper
function getDisplayNameLocal(surfaceForm) {
    const special = {
        " ": "SPACE",
        ".": "FULL STOP",
        ",": "COMMA",
        "?": "QUESTION MARK",
        "!": "EXCLAMATION MARK",
        "'": "APOSTROPHE",
        "\"": "QUOTATION MARK",
        "-": "HYPHEN-MINUS",
        ":": "COLON",
        ";": "SEMICOLON",
        "(": "LEFT PARENTHESIS",
        ")": "RIGHT PARENTHESIS",
        "#": "NUMBER SIGN",
        "$": "DOLLAR SIGN",
        "%": "PERCENT SIGN",
        "&": "AMPERSAND",
        "*": "ASTERISK",
        "+": "PLUS SIGN",
        "/": "SOLIDUS",
        "<": "LESS-THAN SIGN",
        "=": "EQUALS SIGN",
        ">": "GREATER-THAN SIGN",
        "@": "COMMERCIAL AT",
        "[": "LEFT SQUARE BRACKET",
        "\\": "REVERSE SOLIDUS",
        "]": "RIGHT SQUARE BRACKET",
        "^": "CIRCUMFLEX ACCENT",
        "_": "LOW LINE",
        "`": "GRAVE ACCENT",
        "{": "LEFT CURLY BRACKET",
        "|": "VERTICAL LINE",
        "}": "RIGHT CURLY BRACKET",
        "~": "TILDE"
    };
    if (special[surfaceForm]) return special[surfaceForm];
    if (/[0-9]/.test(surfaceForm)) return "DIGIT " + ["ZERO", "ONE", "TWO", "THREE", "FOUR", "FIVE", "SIX", "SEVEN", "EIGHT", "NINE"][surfaceForm.charCodeAt(0) - 48];
    if (/[A-Z]/.test(surfaceForm)) return "LATIN CAPITAL LETTER " + surfaceForm;
    if (/[a-z]/.test(surfaceForm)) return "LATIN SMALL LETTER " + surfaceForm;
    return "UNKNOWN";
}

// Fetch raw active snapshot/profile object
async function fetchActiveSnapshot() {
    try {
        const res = await fetch("/api/snapshots/active");
        if (!res.ok) throw new Error(await res.text());
        activeSnapshot = await res.json();
        
        // Add a click handler on the active snapshot summary card to show its details
        const summaryCard = document.getElementById("snapshot-summary");
        summaryCard.style.cursor = "pointer";
        summaryCard.onclick = (e) => {
            if (!e.target.classList.contains("cid-badge")) {
                showSnapshotDetails();
            }
        };
    } catch (e) {
        console.error("Failed to load active snapshot details: ", e);
    }
}

// Show symbol details
async function showSymbolDetails(entityId) {
    try {
        const res = await fetch(`/api/symbols/${encodeURIComponent(entityId)}`);
        if (!res.ok) throw new Error(await res.text());
        const data = await res.json();

        // Reveal panel
        document.getElementById("details-section").style.display = "block";
        document.getElementById("grapheme-detail-panel").style.display = "flex";
        document.getElementById("word-detail-panel").style.display = "none";
        
        const displayChar = getDisplayCharLocal(data.surfaceForm);
        document.getElementById("detail-object-title").innerText = `Symbol Revision Details (${displayChar})`;
        document.getElementById("det-entity-id").innerText = data.entityId;
        document.getElementById("det-revision-cid").innerText = data.revisionCid;
        document.getElementById("det-surface-form").innerText = displayChar;
        document.getElementById("det-normalized-form").innerText = getDisplayCharLocal(data.normalizedForm);
        document.getElementById("det-unicode-scalar").innerText = data.unicodeScalars.join(", ");
        document.getElementById("det-script").innerText = data.script;
        document.getElementById("det-case").innerText = data.case;
        document.getElementById("det-codec").innerText = data.codec;
        document.getElementById("det-multihash").innerText = data.multihashFormat;

        // Render formatted JSON
        document.getElementById("det-raw-json").innerText = JSON.stringify({
            schema: "language-graph/grapheme-revision/v1",
            entityId: data.entityId,
            kind: "grapheme",
            surfaceForm: data.surfaceForm,
            normalizedForm: data.normalizedForm,
            normalization: data.normalization,
            unicodeScalars: data.unicodeScalars,
            script: data.script,
            case: data.case,
            previousRevisionCid: null
        }, null, 2);

        // Smooth scroll to details
        document.getElementById("details-section").scrollIntoView({ behavior: 'smooth' });
    } catch (e) {
        showError("Failed to fetch symbol details: " + e.message);
    }
}

// Show snapshot/profile details
function showSnapshotDetails() {
    if (!activeSnapshot) return;

    // Reveal panel
    document.getElementById("details-section").style.display = "block";
    document.getElementById("grapheme-detail-panel").style.display = "flex";
    document.getElementById("word-detail-panel").style.display = "none";


    const isProfile = !!activeSnapshot.profileEntityId;

    document.getElementById("detail-object-title").innerText = isProfile 
        ? "Active Profile Snapshot Details" 
        : "Active Collection Snapshot Details";
    document.getElementById("det-entity-id").innerText = isProfile 
        ? activeSnapshot.profileEntityId 
        : activeSnapshot.collectionEntityId;
    document.getElementById("det-revision-cid").innerText = document.getElementById("snapshot-cid").innerText;
    document.getElementById("det-surface-form").innerText = isProfile ? "Text Profile" : "Ordered Collection";
    document.getElementById("det-normalized-form").innerText = "N/A";
    document.getElementById("det-unicode-scalar").innerText = "N/A";
    document.getElementById("det-script").innerText = isProfile ? "Common" : "Latn";
    document.getElementById("det-case").innerText = "none";
    document.getElementById("det-codec").innerText = "dag-cbor";
    document.getElementById("det-multihash").innerText = "sha2-256";

    document.getElementById("det-raw-json").innerText = JSON.stringify(activeSnapshot, null, 2);

    document.getElementById("details-section").scrollIntoView({ behavior: 'smooth' });
}

// Close details panel
function closeDetails() {
    document.getElementById("details-section").style.display = "none";
}

// Resolve user input text
async function resolveInput() {
    const inputField = document.getElementById("resolve-input");
    const text = inputField.value;

    clearError();

    if (!text) {
        showError("Input text cannot be empty.");
        return;
    }

    try {
        const res = await fetch("/api/resolve", {
            method: "POST",
            headers: {
                "Content-Type": "application/json"
            },
            body: JSON.stringify({ text })
        });

        if (!res.ok) {
            const errData = await res.json();
            let errMsg = errData.error || "Unexpected server error";
            
            // Format descriptive validation errors to match Phase 2.1 Printable ASCII specifications
            if (errMsg.includes("Unsupported character or grapheme:")) {
                const parts = errMsg.replace("Unsupported character or grapheme:", "").split(",").map(p => p.trim());
                const formatted = parts.map(part => {
                    // Expect format like: "'🙂' U+1F642 UNSUPPORTED SYMBOL at position 3"
                    const match = part.match(/'(.+)'\s+(U\+[0-9A-Fa-f]+)/);
                    if (match) {
                        return `${match[1]} ${match[2]} is not supported by the active Printable ASCII Text Profile.`;
                    }
                    return part;
                });
                errMsg = formatted.join("\n");
            }
            throw new Error(errMsg);
        }

        const data = await res.json();
        
        // Show result block
        document.getElementById("resolution-result").style.display = "block";
        document.getElementById("res-input-text").innerText = data.input;
        document.getElementById("res-output-text").innerText = data.output;
        document.getElementById("res-snap-cid").innerText = data.collectionSnapshotCid;

        const tbody = document.getElementById("resolution-trace-body");
        tbody.innerHTML = "";

        data.trace.forEach(step => {
            const tr = document.createElement("tr");
            
            const badgeClass = step.status === "Resolved" ? "badge-resolved" : "badge-reused";
            const displayChar = getDisplayCharLocal(step.inputGrapheme);
            const collectionLabel = step.sourceCollectionEntityId.split(":").pop();

            tr.innerHTML = `
                <td><strong>${step.position}</strong></td>
                <td style="font-size: 1.1rem; font-weight: 600; color: #fff;">${displayChar}</td>
                <td><span style="color: #fff;">${step.displayName}</span></td>
                <td><span class="badge badge-reused" style="font-size: 0.7rem;">${step.category}</span></td>
                <td><span style="font-size: 0.85rem; color: var(--text-secondary);">${collectionLabel}</span></td>
                <td><span style="font-family: monospace; font-size: 0.8rem;">${step.entityId}</span></td>
                <td><span class="cid-badge" style="font-size: 0.75rem;" onclick="copyText('${step.revisionCid}')">${step.revisionCid.substring(0, 8)}...</span></td>
                <td>${getDisplayCharLocal(step.surfaceForm)}</td>
                <td><span class="badge ${badgeClass}">${step.status}</span></td>
            `;
            tbody.appendChild(tr);
        });

    } catch (e) {
        showError(e.message);
        document.getElementById("resolution-result").style.display = "none";
    }
}

// Error notice triggers
function showError(msg) {
    const errorBox = document.getElementById("error-notice");
    errorBox.innerText = msg;
    errorBox.style.display = "block";
}

// Clear error
function clearError() {
    document.getElementById("error-notice").style.display = "none";
}

// --- Phase 3 Written Forms UI Logic ---

let storePage = 0;
const storeLimit = 10;
let isSearchActive = false;

async function fetchStoreSummary() {
    try {
        const res = await fetch("/api/word-stores/english-natural-language-written-forms");
        if (!res.ok) throw new Error(await res.text());
        const data = await res.json();
        
        document.getElementById("store-word-count").innerText = data.savedWordCount;
        if (data.activeSnapshotCid) {
            document.getElementById("store-snapshot-cid").innerHTML = 
                `<span class="cid-badge" onclick="copyText('${data.activeSnapshotCid}')">${data.activeSnapshotCid}</span>`;
        } else {
            document.getElementById("store-snapshot-cid").innerText = "Not yet published";
        }
    } catch (e) {
        console.error("Failed to fetch store summary:", e);
    }
}

async function fetchStoreWords() {
    if (isSearchActive) return;
    try {
        const offset = storePage * storeLimit;
        const res = await fetch(`/api/wordforms?store=english-natural-language-written-forms&limit=${storeLimit}&offset=${offset}`);
        if (!res.ok) throw new Error(await res.text());
        const words = await res.json();
        
        renderBrowserList(words);
        
        // Update page indicator
        document.getElementById("page-indicator").innerText = `Page ${storePage + 1}`;
        document.getElementById("prev-page-btn").disabled = storePage === 0;
        document.getElementById("next-page-btn").disabled = words.length < storeLimit;
    } catch (e) {
        console.error("Failed to fetch store words:", e);
    }
}

function renderBrowserList(words) {
    const tbody = document.getElementById("store-browser-body");
    tbody.innerHTML = "";
    
    if (words.length === 0) {
        const tr = document.createElement("tr");
        tr.innerHTML = `<td colspan="6" style="text-align: center; color: var(--text-secondary);">No written forms saved.</td>`;
        tbody.appendChild(tr);
        return;
    }
    
    words.forEach(word => {
        const tr = document.createElement("tr");
        tr.className = "clickable-row";
        tr.onclick = () => showWordDetails(word.surfaceForm);
        
        tr.innerHTML = `
            <td style="font-size: 1.1rem; font-weight: 600; color: #a5b4fc;">${word.surfaceForm}</td>
            <td><span style="font-family: monospace; font-size: 0.8rem;">${word.entityId}</span></td>
            <td><span class="cid-badge" style="font-size: 0.75rem;" onclick="event.stopPropagation(); copyText('${word.revisionCid}')">${word.revisionCid.substring(0, 8)}...</span></td>
            <td><strong>${word.componentCount}</strong></td>
            <td><span class="badge badge-healthy">Active</span></td>
            <td><button class="btn" style="padding: 0.25rem 0.5rem; font-size: 0.75rem;" onclick="event.stopPropagation(); showWordDetails('${word.surfaceForm}')">Details</button></td>
        `;
        tbody.appendChild(tr);
    });
}

function changePage(dir) {
    storePage += dir;
    if (storePage < 0) storePage = 0;
    fetchStoreWords();
}

async function lookupWordform() {
    const surface = document.getElementById("search-input").value.trim();
    if (!surface) return;
    
    try {
        const res = await fetch(`/api/wordforms/exact?surface=${encodeURIComponent(surface)}`);
        if (res.status === 404) {
            const tbody = document.getElementById("store-browser-body");
            tbody.innerHTML = `<td colspan="6" style="text-align: center; color: var(--danger);">Word '${surface}' not found in store (exact case-sensitive match only).</td>`;
            document.getElementById("prev-page-btn").disabled = true;
            document.getElementById("next-page-btn").disabled = true;
            return;
        }
        if (!res.ok) throw new Error(await res.text());
        const word = await res.json();
        
        isSearchActive = true;
        renderBrowserList([word]);
        
        document.getElementById("page-indicator").innerText = "Search Result";
        document.getElementById("prev-page-btn").disabled = true;
        document.getElementById("next-page-btn").disabled = true;
    } catch (e) {
        console.error("Lookup error:", e);
    }
}

function resetBrowser() {
    document.getElementById("search-input").value = "";
    isSearchActive = false;
    storePage = 0;
    fetchStoreWords();
}

async function previewWordform() {
    const input = document.getElementById("compose-input").value.trim();
    clearComposeError();
    document.getElementById("compose-preview-result").style.display = "none";
    document.getElementById("compose-save-result").style.display = "none";
    
    if (!input) {
        showComposeError("Input word cannot be empty.");
        return;
    }
    
    try {
        const res = await fetch("/api/wordforms/preview", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ text: input })
        });
        if (!res.ok) throw new Error(await res.text());
        const data = await res.json();
        
        if (!data.isEligible) {
            showComposeError(data.validationMessage || "Word is not eligible.");
            return;
        }
        
        document.getElementById("compose-preview-result").style.display = "block";
        document.getElementById("prev-candidate").innerText = data.originalInput;
        document.getElementById("prev-eligibility").innerText = "Eligible";
        document.getElementById("prev-status").innerText = data.isAlreadyStored ? "Already Stored" : "Not stored";
        
        const compTrace = data.components.map(c => c.surfaceForm).join(" → ");
        document.getElementById("prev-composition").innerText = compTrace;
    } catch (e) {
        showComposeError(e.message);
    }
}

async function saveWordform() {
    const input = document.getElementById("compose-input").value.trim();
    clearComposeError();
    document.getElementById("compose-preview-result").style.display = "none";
    document.getElementById("compose-save-result").style.display = "none";
    
    if (!input) {
        showComposeError("Input word cannot be empty.");
        return;
    }
    
    try {
        const res = await fetch("/api/wordforms", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ text: input })
        });
        
        if (!res.ok) {
            const errData = await res.json();
            throw new Error(errData.error || "Failed to save written form");
        }
        
        const data = await res.json();
        
        document.getElementById("compose-save-result").style.display = "block";
        document.getElementById("save-surface").innerText = data.surfaceForm;
        document.getElementById("save-entity-id").innerText = data.entityId;
        document.getElementById("save-revision-cid").innerText = data.revisionCid;
        document.getElementById("save-store").innerText = "English Natural-Language Written Forms";
        
        // Refresh store browser and summary
        fetchStoreSummary();
        resetBrowser();
    } catch (e) {
        showComposeError(e.message);
    }
}

async function showWordDetails(surface) {
    try {
        const res = await fetch(`/api/wordforms/details?surface=${encodeURIComponent(surface)}`);
        if (!res.ok) throw new Error(await res.text());
        const data = await res.json();
        
        document.getElementById("details-section").style.display = "block";
        document.getElementById("grapheme-detail-panel").style.display = "none";
        document.getElementById("word-detail-panel").style.display = "flex";
        
        document.getElementById("word-det-surface").innerText = data.surfaceForm;
        document.getElementById("word-det-entity-id").innerText = data.entityId;
        document.getElementById("word-det-revision-cid").innerText = data.revisionCid;
        document.getElementById("word-det-profile-cid").innerText = data.compositionProfileSnapshotCid;
        
        const tbody = document.getElementById("word-det-components-body");
        tbody.innerHTML = "";
        
        data.components.forEach(comp => {
            const tr = document.createElement("tr");
            tr.innerHTML = `
                <td><strong>${comp.position}</strong></td>
                <td style="font-size: 1.15rem; font-weight: 600; color: #a5b4fc;">${comp.surfaceForm}</td>
                <td><span style="font-family: monospace; font-size: 0.85rem;">${comp.symbolEntityId}</span></td>
                <td><span class="cid-badge" onclick="copyText('${comp.symbolRevisionCid}')">${comp.symbolRevisionCid}</span></td>
            `;
            tbody.appendChild(tr);
        });
        
        document.getElementById("word-det-raw-json").innerText = JSON.stringify({
            schema: "language-graph/written-form-revision/v1",
            entityId: data.entityId,
            kind: "written-form",
            formClass: "natural-language-written-form",
            surfaceForm: data.surfaceForm,
            normalizedForm: data.surfaceForm,
            normalization: "NFC",
            compositionProfileSnapshotCid: data.compositionProfileSnapshotCid,
            components: data.components,
            previousRevisionCid: null
        }, null, 2);
        
        document.getElementById("details-section").scrollIntoView({ behavior: 'smooth' });
    } catch (e) {
        showError("Failed to fetch word details: " + e.message);
    }
}

async function publishStoreSnapshot() {
    try {
        const res = await fetch("/api/word-stores/english-natural-language-written-forms/publish", {
            method: "POST"
        });
        if (!res.ok) throw new Error(await res.text());
        const data = await res.json();
        
        document.getElementById("publish-result-box").style.display = "block";
        document.getElementById("pub-snap-cid").innerText = data.snapshotCid;
        document.getElementById("pub-snap-count").innerText = data.memberCount;
        
        fetchStoreSummary();
    } catch (e) {
        alert("Failed to publish store snapshot: " + e.message);
    }
}

function showComposeError(msg) {
    const errorBox = document.getElementById("compose-error-notice");
    errorBox.innerText = msg;
    errorBox.style.display = "block";
}

function clearComposeError() {
    document.getElementById("compose-error-notice").style.display = "none";
}

