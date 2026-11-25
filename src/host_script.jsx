/*
 * FXSearch - In-Process ExtendScript Engine (ES3 Compatible for AE CC - AE 2027+)
 * Executed directly via AEGP_UtilitySuite6->AEGP_ExecuteScript.
 */
var __FXSearchEngine = (function () {
    function pad4(hex) {
        while (hex.length < 4) {
            hex = "0" + hex;
        }
        return hex;
    }

    function escapeJsonString(str) {
        var s = String(str);
        var out = ['"'];
        for (var i = 0; i < s.length; i++) {
            var ch = s.charAt(i);
            var code = s.charCodeAt(i);
            if (ch === '"') {
                out.push('\\"');
            } else if (ch === "\\") {
                out.push("\\\\");
            } else if (ch === "\b") {
                out.push("\\b");
            } else if (ch === "\f") {
                out.push("\\f");
            } else if (ch === "\n") {
                out.push("\\n");
            } else if (ch === "\r") {
                out.push("\\r");
            } else if (ch === "\t") {
                out.push("\\t");
            } else if (code < 32 || code > 126) {
                out.push("\\u" + pad4(code.toString(16)));
            } else {
                out.push(ch);
            }
        }
        out.push('"');
        return out.join("");
    }

    function toJson(val) {
        if (val === null || val === undefined) {
            return "null";
        }
        var t = typeof val;
        if (t === "boolean") {
            return val ? "true" : "false";
        }
        if (t === "number") {
            return isFinite(val) ? String(val) : "null";
        }
        if (t === "string") {
            return escapeJsonString(val);
        }
        if (val instanceof Array) {
            var arrParts = [];
            for (var i = 0; i < val.length; i++) {
                arrParts.push(toJson(val[i]));
            }
            return "[" + arrParts.join(",") + "]";
        }
        if (t === "object") {
            var objParts = [];
            for (var k in val) {
                if (val.hasOwnProperty(k) && val[k] !== undefined) {
                    objParts.push(escapeJsonString(k) + ":" + toJson(val[k]));
                }
            }
            return "{" + objParts.join(",") + "}";
        }
        return "null";
    }

    function getActiveComp() {
        if (!app.project) {
            return null;
        }
        var item = app.project.activeItem;
        if (item && item instanceof CompItem) {
            return item;
        }
        return null;
    }

    function createAdjustmentLayerInComp(comp) {
        var adj = comp.layers.addSolid(
            [1.0, 1.0, 1.0],
            "Adjustment Layer",
            comp.width,
            comp.height,
            comp.pixelAspect,
            comp.duration
        );
        adj.adjustmentLayer = true;
        adj.label = 5;
        return adj;
    }

    function getContext() {
        var comp = getActiveComp();
        return {
            ok: true,
            aeVersion: app.version || "",
            hasProject: Boolean(app.project),
            activeCompId: comp ? comp.id : 0,
            activeCompName: comp ? comp.name : "",
            selectedLayerCount: comp ? comp.selectedLayers.length : 0
        };
    }

    function getInstalledEffects() {
        var list = [];
        var effects = app.effects;
        if (effects) {
            for (var i = 0; i < effects.length; i++) {
                var fx = effects[i];
                var dName = fx.displayName || "";
                var mName = fx.matchName || "";
                var cat = fx.category || "";
                if (!dName || !mName || cat === "_Obsolete") {
                    continue;
                }
                list.push({
                    displayName: dName,
                    matchName: mName,
                    category: cat || "Other",
                    version: fx.version || ""
                });
            }
        }
        return {
            ok: true,
            aeVersion: app.version || "",
            effects: list
        };
    }

    function applyEffect(payload) {
        var comp = getActiveComp();
        if (!comp) {
            return {
                ok: false,
                error: "请先在 After Effects 中打开并激活一个合成"
            };
        }

        var matchName = payload.matchName || "";
        var displayName = payload.displayName || matchName;
        var forceAdj = Boolean(payload.forceAdjustmentLayer);

        if (!matchName) {
            return { ok: false, error: "缺少效果 matchName" };
        }

        var appliedCount = 0;
        app.beginUndoGroup("FXSearch: " + displayName);
        try {
            var targets = [];
            if (forceAdj || comp.selectedLayers.length === 0) {
                targets.push(createAdjustmentLayerInComp(comp));
            } else {
                for (var i = 0; i < comp.selectedLayers.length; i++) {
                    targets.push(comp.selectedLayers[i]);
                }
            }

            var selProps = comp.selectedProperties;
            for (var p = selProps.length - 1; p >= 0; p--) {
                try {
                    selProps[p].selected = false;
                } catch (e1) {}
            }

            for (var t = 0; t < targets.length; t++) {
                var layer = targets[t];
                var parade = layer.property("ADBE Effect Parade");
                if (parade && parade.canAddProperty(matchName)) {
                    var added = parade.addProperty(matchName);
                    if (added) {
                        try {
                            added.selected = true;
                        } catch (e2) {}
                        appliedCount++;
                    }
                }
            }
        } catch (err) {
            return { ok: false, error: "应用效果失败: " + String(err.message || err) };
        } finally {
            app.endUndoGroup();
        }

        if (appliedCount === 0) {
            return { ok: false, error: "当前选中图层不支持添加该效果（如摄像机或灯光层）" };
        }
        return { ok: true, appliedCount: appliedCount };
    }

    function applyPreset(payload) {
        var comp = getActiveComp();
        if (!comp) {
            return { ok: false, error: "请先在 After Effects 中打开并激活一个合成" };
        }

        var filePath = payload.filePath || "";
        var presetName = payload.presetName || "Preset";
        var forceAdj = Boolean(payload.forceAdjustmentLayer);
        var presetFile = new File(filePath);
        if (!presetFile.exists) {
            return { ok: false, error: "找不到预设文件: " + filePath };
        }

        var appliedCount = 0;
        app.beginUndoGroup("FXSearch Preset: " + presetName);
        try {
            var targets = [];
            if (forceAdj || comp.selectedLayers.length === 0) {
                targets.push(createAdjustmentLayerInComp(comp));
            } else {
                for (var i = 0; i < comp.selectedLayers.length; i++) {
                    targets.push(comp.selectedLayers[i]);
                }
            }

            for (var s = comp.selectedLayers.length - 1; s >= 0; s--) {
                comp.selectedLayers[s].selected = false;
            }

            for (var t = 0; t < targets.length; t++) {
                var target = targets[t];
                for (var c = comp.selectedLayers.length - 1; c >= 0; c--) {
                    comp.selectedLayers[c].selected = false;
                }
                target.selected = true;
                target.applyPreset(presetFile);
                appliedCount++;
            }

            for (var r = 0; r < targets.length; r++) {
                targets[r].selected = true;
            }
        } catch (err) {
            return { ok: false, error: "应用预设失败: " + String(err.message || err) };
        } finally {
            app.endUndoGroup();
        }

        return { ok: true, appliedCount: appliedCount };
    }

    function scanUsedEffects() {
        if (!app.project) {
            return { ok: true, activeCompId: 0, activeCompName: "", items: [] };
        }

        var activeComp = getActiveComp();
        var activeCompId = activeComp ? activeComp.id : 0;
        var activeCompName = activeComp ? activeComp.name : "";

        var items = app.project.items;
        var totalItems = items.length;
        var results = [];

        for (var c = 1; c <= totalItems; c++) {
            var projItem = items[c];
            if (!(projItem instanceof CompItem)) {
                continue;
            }
            var isAct = projItem.id === activeCompId;
            var numLayers = projItem.numLayers;

            for (var l = 1; l <= numLayers; l++) {
                var layer = projItem.layer(l);
                if (layer instanceof CameraLayer || layer instanceof LightLayer) {
                    continue;
                }
                var parade = layer.property("ADBE Effect Parade");
                if (!parade || parade.numProperties === 0) {
                    continue;
                }
                var numFx = parade.numProperties;
                for (var e = 1; e <= numFx; e++) {
                    var fx = parade.property(e);
                    if (!fx) {
                        continue;
                    }
                    var fxEnabled = true;
                    try {
                        if (fx.canSetEnabled) {
                            fxEnabled = fx.enabled;
                        }
                    } catch (errEn) {}

                    results.push({
                        compId: projItem.id,
                        compName: projItem.name,
                        isActiveComp: isAct,
                        layerIndex: l,
                        layerName: layer.name,
                        layerLabel: layer.label || 0,
                        effectIndex: e,
                        effectName: fx.name || "",
                        matchName: fx.matchName || "",
                        effectEnabled: fxEnabled
                    });
                }
            }
        }

        return {
            ok: true,
            activeCompId: activeCompId,
            activeCompName: activeCompName,
            items: results
        };
    }

    function findCompByIdOrName(compId, compName) {
        if (!app.project) {
            return null;
        }
        if (compId) {
            var byId = app.project.itemByID(Number(compId));
            if (byId && byId instanceof CompItem) {
                return byId;
            }
        }
        if (compName) {
            for (var i = 1; i <= app.project.numItems; i++) {
                var it = app.project.items[i];
                if (it instanceof CompItem && it.name === compName) {
                    return it;
                }
            }
        }
        return null;
    }

    function revealUsedEffect(payload) {
        var comp = findCompByIdOrName(payload.compId, payload.compName);
        if (!comp) {
            return { ok: false, error: "未找到目标合成" };
        }

        var subAction = payload.subAction || "reveal";
        var layerIdx = Number(payload.layerIndex) || 1;
        var effectIdx = Number(payload.effectIndex) || 1;
        var matchName = payload.matchName || "";
        var effectName = payload.effectName || "";

        if (layerIdx < 1 || layerIdx > comp.numLayers) {
            return { ok: false, error: "目标图层索引超出范围" };
        }

        var targetLayer = comp.layer(layerIdx);
        var parade = targetLayer.property("ADBE Effect Parade");
        var targetFx = null;

        if (parade && parade.numProperties > 0) {
            if (effectIdx >= 1 && effectIdx <= parade.numProperties) {
                var candidate = parade.property(effectIdx);
                if (candidate && (!matchName || candidate.matchName === matchName)) {
                    targetFx = candidate;
                }
            }
            if (!targetFx) {
                for (var i = 1; i <= parade.numProperties; i++) {
                    var prop = parade.property(i);
                    if (
                        prop &&
                        ((matchName && prop.matchName === matchName) ||
                            (effectName && prop.name === effectName))
                    ) {
                        targetFx = prop;
                        break;
                    }
                }
            }
        }

        if (subAction === "toggle") {
            if (!targetFx || !targetFx.canSetEnabled) {
                return { ok: false, error: "该效果不支持启用/禁用切换" };
            }
            var newState = !targetFx.enabled;
            app.beginUndoGroup("FXSearch: Toggle " + targetFx.name);
            try {
                targetFx.enabled = newState;
            } finally {
                app.endUndoGroup();
            }
            return { ok: true, toggled: true, effectEnabled: newState };
        }

        app.beginUndoGroup("FXSearch: Locate Effect");
        try {
            var viewer = comp.openInViewer();
            if (viewer) {
                try {
                    viewer.setActive();
                } catch (eViewer) {}
            }

            if (targetLayer.shy && comp.hideShyLayers) {
                comp.hideShyLayers = false;
            }
            if (targetLayer.locked) {
                targetLayer.locked = false;
            }

            for (var s = comp.selectedLayers.length - 1; s >= 0; s--) {
                try {
                    comp.selectedLayers[s].selected = false;
                } catch (eSelL) {}
            }
            var selProps = comp.selectedProperties;
            for (var p = selProps.length - 1; p >= 0; p--) {
                try {
                    selProps[p].selected = false;
                } catch (eSelP) {}
            }

            targetLayer.selected = true;
            if (targetFx) {
                try {
                    targetFx.selected = true;
                } catch (eFxSel) {}
            }

            try {
                app.executeCommand(2771);
            } catch (eCmd1) {}

            try {
                if (targetFx) {
                    targetFx.selected = true;
                }
            } catch (eReSel) {}

            try {
                app.executeCommand(3734);
            } catch (eCmd2) {}
        } catch (err) {
            return { ok: false, error: "定位图层效果失败: " + String(err.message || err) };
        } finally {
            app.endUndoGroup();
        }

        return { ok: true };
    }

    function dispatch(req) {
        try {
            var action = req.action || "getContext";
            var payload = req.payload || {};
            var res;
            if (action === "getContext") res = getContext();
            else if (action === "getInstalledEffects") res = getInstalledEffects();
            else if (action === "applyEffect") res = applyEffect(payload);
            else if (action === "applyPreset") res = applyPreset(payload);
            else if (action === "scanUsedEffects") res = scanUsedEffects();
            else if (action === "revealUsedEffect") res = revealUsedEffect(payload);
            else res = { ok: false, error: "Unknown action: " + action };
            return toJson(res);
        } catch (e) {
            return toJson({ ok: false, error: String(e.message || e) });
        }
    }

    return { dispatch: dispatch };
})();
