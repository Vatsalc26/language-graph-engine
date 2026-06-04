// Global state cache
let activeSnapshot = null;
let currentSymbolList = [];

// Init on load
document.addEventListener("DOMContentLoaded", () => {
    fetchStatus();
    fetchCollections();
    fetchSymbols();
    fetchActiveSnapshot();
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
