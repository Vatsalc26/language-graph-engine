// Global state cache
let activeSnapshot = null;
let currentSymbolList = [];

// Init on load
document.addEventListener("DOMContentLoaded", () => {
    fetchStatus();
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

// Fetch 26 symbols
async function fetchSymbols() {
    try {
        const res = await fetch("/api/symbols");
        if (!res.ok) throw new Error(await res.text());
        const symbols = await res.json();
        currentSymbolList = symbols;

        // Render collection label on status block
        if (symbols.length > 0) {
            document.getElementById("collection-name").innerText = "Latin lowercase alphabet a-z";
        }

        const tbody = document.getElementById("symbol-registry-body");
        tbody.innerHTML = "";

        symbols.forEach(sym => {
            const tr = document.createElement("tr");
            tr.className = "clickable-row";
            tr.onclick = () => showSymbolDetails(sym.canonicalEntityId);
            
            tr.innerHTML = `
                <td><strong>${sym.position}</strong></td>
                <td style="font-size: 1.15rem; font-weight: 600; color: #a5b4fc;">${sym.surfaceForm}</td>
                <td><span style="font-family: monospace; font-size: 0.85rem;">${sym.canonicalEntityId}</span></td>
                <td><span class="cid-badge" style="font-size: 0.8rem;">${sym.activeRevisionCid.substring(0, 8)}...</span></td>
                <td><span class="badge badge-reused">${sym.normalization}</span></td>
                <td><span class="badge badge-healthy">Healthy</span></td>
            `;
            tbody.appendChild(tr);
        });
    } catch (e) {
        showError("Failed to load symbols registry: " + e.message);
    }
}

// Fetch raw active snapshot object
async function fetchActiveSnapshot() {
    try {
        const res = await fetch("/api/snapshots/active");
        if (!res.ok) throw new Error(await res.text());
        activeSnapshot = await res.json();
        
        // Add a click handler on the active snapshot summary card to show its details
        const summaryCard = document.getElementById("snapshot-summary");
        summaryCard.style.cursor = "pointer";
        summaryCard.onclick = (e) => {
            // Prevent drawer close if badge clicked
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

        document.getElementById("detail-object-title").innerText = `Symbol Revision Details (${data.surfaceForm})`;
        document.getElementById("det-entity-id").innerText = data.entityId;
        document.getElementById("det-revision-cid").innerText = data.revisionCid;
        document.getElementById("det-surface-form").innerText = data.surfaceForm;
        document.getElementById("det-normalized-form").innerText = data.normalizedForm;
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

// Show snapshot details
function showSnapshotDetails() {
    if (!activeSnapshot) return;

    // Reveal panel
    document.getElementById("details-section").style.display = "block";
    document.getElementById("grapheme-detail-panel").style.display = "flex";

    document.getElementById("detail-object-title").innerText = "Active Collection Snapshot Details";
    document.getElementById("det-entity-id").innerText = activeSnapshot.collectionEntityId;
    document.getElementById("det-revision-cid").innerText = document.getElementById("snapshot-cid").innerText;
    document.getElementById("det-surface-form").innerText = "Ordered Collection";
    document.getElementById("det-normalized-form").innerText = "N/A";
    document.getElementById("det-unicode-scalar").innerText = "N/A";
    document.getElementById("det-script").innerText = "Latn";
    document.getElementById("det-case").innerText = "lowercase";
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

    // Client-side quick check for empty
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
            throw new Error(errData.error || "Unexpected server error");
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
            tr.innerHTML = `
                <td><strong>${step.position}</strong></td>
                <td style="font-size: 1.1rem; font-weight: 600; color: #fff;">${step.inputGrapheme}</td>
                <td><span style="font-family: monospace; font-size: 0.8rem;">${step.entityId}</span></td>
                <td><span class="cid-badge" style="font-size: 0.75rem;">${step.revisionCid.substring(0, 8)}...</span></td>
                <td>${step.surfaceForm}</td>
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

function clearError() {
    document.getElementById("error-notice").style.display = "none";
}
