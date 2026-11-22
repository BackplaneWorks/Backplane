package dev.backplane.mobile

import android.graphics.Paint
import android.graphics.Typeface
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Fill
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.input.pointer.changedToUp
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlin.math.abs
import kotlin.math.hypot
import kotlin.math.max
import kotlin.math.min
import kotlinx.coroutines.launch
import org.json.JSONArray
import org.json.JSONObject

private const val PCB_SUBSTRATE = 0xFF111917L
private const val PCB_BOARD_FILL = 0xFF16201DL
private const val PCB_EDGE = 0xFFD4E0DBL
private const val PCB_PART = 0xFF7A9188L
private const val PCB_OVERHANG = 0xFFF59E0BL
private const val PCB_REFDES = 0xBB8FA39BL
private const val PCB_BADGE_BG = 0xF0111917L
private const val PCB_BADGE_FG = 0xFFEEF5F2L
private const val PCB_BAD_RED = 0xFFEF4444L

private fun rgbColor(rgb: Int, alpha: Float = 1f): Color {
    val a = (alpha.coerceIn(0f, 1f) * 255f).toInt()
    return Color((a shl 24) or (rgb and 0x00FFFFFF))
}

private data class PendingPlanEdit(
    val seq: Int,
    val pro: String,
    val itemId: String,
    val patch: PlanOptimisticPatch,
    val onlyGeom: String? = null,
    val committedRev: Int? = null,
)

@Composable
internal fun PlanFloorplannerPanel(fallbackProject: String) {
    val scope = rememberCoroutineScope()
    val rawObj = DataClient.mgmtData("plan")
    val basePanel = remember(rawObj) { PlanFloorplannerModel.unwrapPanel(rawObj) }

    var activeLayer by remember(fallbackProject) { mutableStateOf("") }
    var selectedId by remember(fallbackProject) { mutableStateOf("") }
    var selectedVertex by remember(fallbackProject) { mutableStateOf<Pair<String, Int>?>(null) }
    var inspectorOpen by remember(fallbackProject) { mutableStateOf(false) }
    var stackupOpen by remember(fallbackProject) { mutableStateOf(false) }
    var cam by remember(fallbackProject) { mutableStateOf<PlanCamera?>(null) }
    var cameraIsFitted by remember(fallbackProject) { mutableStateOf(true) }
    var canvasPx by remember { mutableStateOf(IntSize.Zero) }

    var hydratedKey by remember(fallbackProject) { mutableStateOf("") }
    var hydratedRev by remember(fallbackProject) { mutableIntStateOf(-1) }
    var hydratedBoard by remember(fallbackProject) { mutableStateOf<PlanBoardGeom?>(null) }
    var hydratedDraw by remember(fallbackProject) { mutableStateOf<PlanDraw?>(null) }
    var hydratedLayers by remember(fallbackProject) { mutableStateOf<List<PlanLayer>?>(null) }

    var pendingEdits by remember(fallbackProject) { mutableStateOf<List<PendingPlanEdit>>(emptyList()) }
    var nextEditSeq by remember(fallbackProject) { mutableIntStateOf(1) }
    var drainingEdits by remember(fallbackProject) { mutableStateOf(false) }
    var ackedRev by remember(fallbackProject) { mutableIntStateOf(0) }
    var dragPreview by remember(fallbackProject) { mutableStateOf<Pair<String, PlanOptimisticPatch>?>(null) }
    var undoStack by remember(fallbackProject) { mutableStateOf<List<PlanUndoEntry>>(emptyList()) }
    var errMsg by remember(fallbackProject) { mutableStateOf("") }
    var lastCanonicalPro by remember(fallbackProject) { mutableStateOf("") }
    val revRef = remember(fallbackProject) { intArrayOf(0) }

    var dragWorld by remember { mutableStateOf<Pair<Double, Double>?>(null) }
    var dragScreen by remember { mutableStateOf<Offset?>(null) }
    var lastTapAt by remember { mutableLongStateOf(0L) }
    var lastTapPos by remember { mutableStateOf(Offset.Zero) }

    if (basePanel == null) {
        Column(Modifier.fillMaxSize().padding(14.dp)) {
            DMono(Msg.gpui_mgmt_plan_reading, dfaint, 12)
        }
        return
    }

    val pro = basePanel.pro.ifEmpty { DataClient.mgmtProject.ifEmpty { fallbackProject } }
    val authBaseRev = max(basePanel.revision, if (hydratedKey.startsWith("$pro:") && hydratedRev > basePanel.revision) hydratedRev else 0)
    revRef[0] = authBaseRev

    LaunchedEffect(pro) {
        if (lastCanonicalPro.isNotEmpty() && lastCanonicalPro != pro) {
            dragPreview = null
            dragWorld = null
            dragScreen = null
            selectedId = ""
            selectedVertex = null
            inspectorOpen = false
            cameraIsFitted = true
            cam = null
            hydratedKey = ""
            hydratedRev = -1
            hydratedBoard = null
            hydratedDraw = null
            hydratedLayers = null
            ackedRev = basePanel.revision
        }
        lastCanonicalPro = pro
    }

    LaunchedEffect(basePanel.revision, pro) {
        ackedRev = max(ackedRev, basePanel.revision)
        pendingEdits = pendingEdits.filterNot { it.pro == pro && (it.committedRev ?: Int.MAX_VALUE) <= basePanel.revision }
    }

    suspend fun refreshAuthoritativePlan(targetPro: String) {
        if (DataClient.conn != DataClient.Conn.ONLINE || targetPro.isEmpty()) return
        runCatching {
            val reply = DataClient.planGet(targetPro, board = true)
            PlanFloorplannerModel.unwrapPanel(reply)?.let { full ->
                if (full.revision >= hydratedRev) {
                    hydratedKey = "$targetPro:${full.revision}"
                    hydratedRev = full.revision
                    hydratedBoard = full.board
                    hydratedDraw = full.draw
                    hydratedLayers = full.layers.takeIf { it.isNotEmpty() }
                    revRef[0] = max(revRef[0], full.revision)
                    ackedRev = max(ackedRev, full.revision)
                    pendingEdits = pendingEdits.filterNot { it.pro == targetPro && (it.committedRev ?: Int.MAX_VALUE) <= full.revision }
                }
            }
        }
    }

    LaunchedEffect(pro, basePanel.revision, basePanel.boardTrimmed, DataClient.conn) {
        if (DataClient.conn != DataClient.Conn.ONLINE || pro.isEmpty()) return@LaunchedEffect
        val key = "$pro:${basePanel.revision}"
        val needHydrate = basePanel.boardTrimmed || basePanel.board == null
        if (!needHydrate) {
            hydratedKey = key
            hydratedRev = basePanel.revision
            hydratedBoard = null
            hydratedDraw = null
            hydratedLayers = null
            ackedRev = max(ackedRev, basePanel.revision)
            return@LaunchedEffect
        }
        if (hydratedKey == key && hydratedBoard != null) return@LaunchedEffect
        refreshAuthoritativePlan(pro)
    }

    val panel = remember(basePanel, hydratedBoard, hydratedDraw, hydratedLayers, hydratedKey, hydratedRev, pendingEdits, dragPreview, pro) {
        val withHydrated = if (hydratedKey.startsWith("$pro:") && hydratedRev >= basePanel.revision) {
            basePanel.copy(
                board = hydratedBoard ?: basePanel.board,
                draw = hydratedDraw ?: basePanel.draw,
                layers = if (hydratedRev > basePanel.revision && hydratedLayers != null) hydratedLayers!! else basePanel.layers,
                revision = max(basePanel.revision, hydratedRev),
            )
        } else {
            basePanel
        }
        val folded = PlanFloorplannerModel.foldPatches(
            pendingEdits.filter { it.pro == pro }.map { it.itemId to it.patch },
            preview = dragPreview,
        )
        PlanFloorplannerModel.mergeOptimistic(withHydrated, folded)
    }

    val layers = remember(panel.layers, panel.copperLayers) {
        PlanFloorplannerModel.allCanvasLayers(panel.layers, panel.copperLayers)
    }
    val curLayer = if (layers.any { it.layer == activeLayer }) activeLayer else PlanFloorplannerModel.defaultActiveLayer(layers)
    val layerObj = layers.firstOrNull { it.layer == curLayer }
    val board = panel.board ?: PlanBoardGeom()
    val draw = panel.draw ?: PlanDraw()

    fun captureUndoSnapshot(id: String, patch: PlanOptimisticPatch): PlanOptimisticPatch {
        val band = panel.draw?.bands?.firstOrNull { it.id == id }
        val region = panel.draw?.regions?.firstOrNull { it.id == id }
        val found = findPlanItem(id, panel.layers)
        return PlanOptimisticPatch(
            path = if (patch.path != null) band?.path else null,
            outline = if (patch.outline != null) region?.outline else null,
            width = if (patch.width != null) (found?.second?.width ?: band?.width ?: 1.5) else null,
            layer = if (patch.layer != null) (found?.first ?: band?.layer ?: region?.layer) else null,
            locked = if (patch.locked != null) (found?.second?.locked ?: band?.locked ?: region?.locked ?: false) else null,
        )
    }

    fun drainPendingEditsIfNeeded(fallbackBaseRev: Int) {
        if (drainingEdits) return
        drainingEdits = true
        scope.launch {
            try {
                while (true) {
                    val edit = pendingEdits.firstOrNull { it.committedRev == null } ?: break
                    val fields = JSONObject()
                    if (edit.onlyGeom == "path" || (edit.onlyGeom == null && edit.patch.path != null)) {
                        edit.patch.path?.let { pts -> fields.put("path", JSONArray(pts.map { JSONArray(listOf(it.first, it.second)) })) }
                    }
                    if (edit.onlyGeom == "outline" || (edit.onlyGeom == null && edit.patch.outline != null)) {
                        edit.patch.outline?.let { pts -> fields.put("outline", JSONArray(pts.map { JSONArray(listOf(it.first, it.second)) })) }
                    }
                    edit.patch.width?.let { fields.put("width", it) }
                    edit.patch.layer?.let { fields.put("layer", it) }
                    edit.patch.locked?.let { fields.put("locked", it) }
                    if (fields.length() == 0) {
                        pendingEdits = pendingEdits.filterNot { it.seq == edit.seq }
                        continue
                    }
                    val sendBase = max(max(fallbackBaseRev, revRef[0]), ackedRev)
                    val op = JSONObject().put("op", "set").put("id", edit.itemId).put("fields", fields)
                    runCatching { DataClient.planUpdate(edit.pro, listOf(op), sendBase) }
                        .onSuccess { reply ->
                            val replyRev = reply.optInt("revision", 0)
                            val targetRev = if (replyRev > sendBase) replyRev else (sendBase + 1)
                            ackedRev = max(ackedRev, targetRev)
                            pendingEdits = pendingEdits
                                .map { if (it.seq == edit.seq) it.copy(committedRev = targetRev) else it }
                                .filterNot { it.pro == edit.pro && (it.committedRev ?: Int.MAX_VALUE) <= revRef[0] }
                        }
                        .onFailure { err ->
                            pendingEdits = pendingEdits.filterNot { it.seq == edit.seq }
                            errMsg = err.message ?: Msg.android_failed_generic
                            refreshAuthoritativePlan(edit.pro)
                        }
                }
            } finally {
                drainingEdits = false
            }
        }
    }

    fun enqueueEdit(
        targetPro: String,
        id: String,
        patch: PlanOptimisticPatch,
        undoPrev: PlanOptimisticPatch?,
        baseRev: Int,
        onlyGeom: String? = null,
    ) {
        if (undoPrev != null) {
            undoStack = undoStack + PlanUndoEntry(targetPro, id, undoPrev, onlyGeom)
        }
        val seq = nextEditSeq
        nextEditSeq += 1
        pendingEdits = pendingEdits + PendingPlanEdit(seq, targetPro, id, patch, onlyGeom, committedRev = null)
        ackedRev = max(ackedRev, baseRev)
        drainPendingEditsIfNeeded(baseRev)
    }

    val density = LocalDensity.current
    val dpScale = density.density.coerceAtLeast(1f)
    val canvasDpW = canvasPx.width.toDouble() / dpScale
    val canvasDpH = canvasPx.height.toDouble() / dpScale

    val fitCameraNow: () -> Unit = {
        cameraIsFitted = true
        if (canvasDpW > 10.0 && canvasDpH > 10.0) {
            cam = PlanCamera.fit(board.bbox, canvasDpW, canvasDpH, 22.0)
        } else {
            cam = null
        }
    }

    LaunchedEffect(curLayer, canvasPx, dpScale, board.bbox, cameraIsFitted) {
        if ((cameraIsFitted || cam == null) && canvasDpW > 10.0 && canvasDpH > 10.0) {
            cam = PlanCamera.fit(board.bbox, canvasDpW, canvasDpH, 22.0)
        }
    }

    val (stLabel, stColor) = planStatusMeta(panel)

    Column(Modifier.fillMaxSize().background(dcs.background)) {
        // Merged top header + On/Off + flush 3dp progress bar
        Column(Modifier.fillMaxWidth().background(dcs.surfaceVariant)) {
            Row(
                Modifier.fillMaxWidth().padding(horizontal = 10.dp, vertical = 6.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                DMono(Msg.gpui_mgmt_plan_before, dcs.onSurface, 11, bold = true)
                if (!panel.empty) {
                    DMono(stLabel, stColor, 10, Modifier.border(BorderStroke(1.dp, stColor)).padding(horizontal = 4.dp, vertical = 1.dp), bold = true)
                    if (panel.revision > 0) {
                        val revTag = "v" + panel.revision
                        DMono(revTag, dfaint, 10)
                    }
                    val countStr = when {
                        panel.total > 0 -> Msg.gpui_mgmt_plan_done_of(panel.done, panel.total)
                        panel.awaiting > 0 -> Msg.gpui_mgmt_plan_awaiting_short(panel.awaiting)
                        else -> ""
                    }
                    if (countStr.isNotEmpty()) {
                        DMono(countStr, if (panel.awaiting > 0) dwarn else dcs.onSurfaceVariant, 10, Modifier.weight(1f))
                    } else {
                        Spacer(Modifier.weight(1f))
                    }
                } else {
                    Spacer(Modifier.weight(1f))
                }
                PlanMiniChip(Msg.gpui_bots_recall_on, panel.on) { scope.launch { runCatching { DataClient.planEnable(pro, true) } } }
                PlanMiniChip(Msg.gpui_bots_recall_off, !panel.on) { scope.launch { runCatching { DataClient.planEnable(pro, false) } } }
            }
            if (!panel.empty && (panel.awaiting > 0 || panel.routing || (panel.live && panel.done > 0))) {
                Row(Modifier.fillMaxWidth().padding(start = 10.dp, end = 10.dp, bottom = 6.dp), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    if (panel.awaiting > 0) {
                        DBtn(Msg.gpui_mgmt_plan_all, true, small = true, modifier = Modifier.testTag("plan.approveAll")) {
                            scope.launch {
                                runCatching { DataClient.planApprove(pro, "", emptyList(), true) }
                                    .onFailure { errMsg = it.message ?: Msg.android_failed_generic }
                            }
                        }
                    }
                    if (panel.routing) {
                        DBtn(Msg.gpui_mgmt_plan_stop, false, small = true, danger = true) {
                            scope.launch { runCatching { DataClient.planStop(pro) } }
                        }
                    }
                    if (panel.live && panel.done > 0) {
                        DBtn(Msg.gpui_mgmt_plan_verify, false, small = true) {
                            scope.launch { runCatching { DataClient.planVerify(pro) } }
                        }
                    }
                }
            }
            if (!panel.empty && panel.total > 0) {
                PlanProgressBar(panel)
            }
        }
        DLine()

        if (errMsg.isNotEmpty()) {
            Row(Modifier.fillMaxWidth().background(dcs.surfaceVariant).padding(horizontal = 10.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                DMono(Tr.t(errMsg), dcs.error, 11, Modifier.weight(1f), lines = 2)
                Icon(Icons.Default.Close, Msg.common_close, tint = dcs.onSurfaceVariant, modifier = Modifier.size(16.dp).clickable { errMsg = "" })
            }
            DLine()
        }

        if (panel.empty) {
            Column(Modifier.fillMaxSize().padding(14.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                DMono(Msg.gpui_mgmt_plan_noboard, dcs.onSurfaceVariant, 12, bold = true)
                DMono(Msg.gpui_mgmt_plan_emptysub, dfaint, 11, lines = 3)
            }
            return@Column
        }

        // Single horizontal canonical layer chip bar
        Row(
            Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal = 10.dp, vertical = 5.dp),
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            layers.forEach { l ->
                val active = l.layer == curLayer
                val emptyL = l.items.isEmpty()
                val title = PlanFloorplannerModel.layerDisplayTitle(l.layer, panel.copperLayers, layers)
                val label = if (emptyL) title else "$title · ${l.items.size}"
                Box(
                    Modifier
                        .height(30.dp)
                        .background(if (active) dcs.primary else dcs.surfaceVariant)
                        .border(BorderStroke(1.dp, if (active) dcs.primary else dcs.outlineVariant))
                        .clickable { activeLayer = l.layer; selectedVertex = null }
                        .padding(horizontal = 10.dp)
                        .testTag("plan.layer.${l.layer}"),
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        label,
                        color = (if (active) dcs.onPrimary else if (emptyL) dfaint else dcs.onSurfaceVariant).copy(alpha = if (emptyL && !active) 0.65f else 1f),
                        fontFamily = BpMono,
                        fontSize = 11.sp,
                        fontWeight = if (active) FontWeight.Bold else FontWeight.Normal,
                        maxLines = 1,
                    )
                }
            }
        }
        DLine()

        // Slim 1-line stackup / Fit / per-layer approve strip
        val hasAwaiting = layerObj?.items?.any { it.state == "proposed" || it.state == "edited" } == true
        val subtitle = layerObj?.subtitle.orEmpty().ifEmpty { layerObj?.stackup.orEmpty() }
        Row(
            Modifier.fillMaxWidth().background(dcs.surfaceVariant).padding(horizontal = 10.dp, vertical = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            if (subtitle.isNotEmpty()) DMono(subtitle, dfaint, 10, Modifier.weight(1f)) else Spacer(Modifier.weight(1f))
            if (panel.stackupSummary.isNotEmpty() || panel.impedances.isNotEmpty()) {
                val stTxt = Msg.gpui_mgmt_plan_stackup_short + (if (stackupOpen) " ▾" else " ▸")
                PlanMiniChip(stTxt, stackupOpen) { stackupOpen = !stackupOpen }
            }
            PlanMiniChip(Msg.gpui_mgmt_plan_fit, false, onClick = fitCameraNow)
            if (hasAwaiting) {
                Box(
                    Modifier.size(28.dp).background(dcs.surface).border(BorderStroke(1.dp, dcs.outlineVariant))
                        .clickable { scope.launch { runCatching { DataClient.planApprove(pro, curLayer, emptyList(), true) } } },
                    contentAlignment = Alignment.Center,
                ) { DMono("✓", dok, 12, bold = true) }
                Box(
                    Modifier.size(28.dp).background(dcs.surface).border(BorderStroke(1.dp, dcs.outlineVariant))
                        .clickable { scope.launch { runCatching { DataClient.planApprove(pro, curLayer, emptyList(), false) } } },
                    contentAlignment = Alignment.Center,
                ) { DMono("✗", dcs.error, 12, bold = true) }
            }
        }
        if (stackupOpen && (panel.stackupSummary.isNotEmpty() || panel.impedances.isNotEmpty())) {
            Column(Modifier.fillMaxWidth().background(dcs.background).padding(horizontal = 12.dp, vertical = 6.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                if (panel.stackupSummary.isNotEmpty()) DMono(panel.stackupSummary, dcs.onSurfaceVariant, 10, lines = 3)
                panel.impedances.forEach { imp -> DMono("· $imp", dfaint, 10) }
            }
        }
        DLine()

        // Interactive darkroom PCB canvas (all PlanCamera & hitTest math stays in density-independent dp)
        val fittedCam = if (canvasDpW > 10.0 && canvasDpH > 10.0) {
            PlanCamera.fit(board.bbox, canvasDpW, canvasDpH, 22.0)
        } else {
            PlanCamera.fit(board.bbox, 360.0, 260.0, 22.0)
        }
        val activeCam = if (cameraIsFitted) fittedCam else (cam ?: fittedCam)
        val stDraw by rememberUpdatedState(draw)
        val stBoard by rememberUpdatedState(board)
        val stLayers by rememberUpdatedState(layers)
        val stSelectedId by rememberUpdatedState(selectedId)
        val stCanvasDpW by rememberUpdatedState(canvasDpW)
        val stCanvasDpH by rememberUpdatedState(canvasDpH)
        val stDpScale by rememberUpdatedState(dpScale)
        val stActiveCam by rememberUpdatedState(activeCam)
        val stBaseRev by rememberUpdatedState(panel.revision)
        val touchSlopDp = 8f

        Box(
            Modifier
                .fillMaxWidth()
                .weight(1f)
                .background(Color(PCB_SUBSTRATE))
                .onSizeChanged { sz ->
                    canvasPx = sz
                    val dw = sz.width.toDouble() / dpScale
                    val dh = sz.height.toDouble() / dpScale
                    if ((cameraIsFitted || cam == null) && dw > 10.0 && dh > 10.0) {
                        cam = PlanCamera.fit(board.bbox, dw, dh, 22.0)
                    }
                }
                .pointerInput(pro, curLayer) {
                    awaitEachGesture {
                        val first = awaitFirstDown(requireUnconsumed = false)
                        val gScale = stDpScale
                        val w = stCanvasDpW.coerceAtLeast(1.0)
                        val h = stCanvasDpH.coerceAtLeast(1.0)
                        val gDraw = stDraw
                        val gBoard = stBoard
                        val gLayers = stLayers
                        val gSel = stSelectedId
                        val startCam = stActiveCam
                        val gBaseRev = stBaseRev
                        val toDp: (Offset) -> Offset = { p -> Offset(p.x / gScale, p.y / gScale) }

                        val downPos = toDp(first.position)
                        val downTime = System.currentTimeMillis()
                        var moved = false
                        var wasMulti = false
                        var liftedNormally = false
                        var prevSpan = 0f
                        var prevCentroid = Offset.Zero
                        var multiCam = startCam

                        val startHit = PlanFloorplannerModel.hitTest(
                            gDraw.bands, gDraw.regions, curLayer, gSel, startCam, w, h,
                            downPos.x.toDouble(), downPos.y.toDouble(), 24.0, 20.0,
                        )
                        var dragVertex: PlanHit.Vertex? = null
                        var vertexGrab = Offset.Zero
                        var vertexPts: List<Pair<Double, Double>> = emptyList()
                        var latestVertexPts: List<Pair<Double, Double>>? = null
                        var totalPan = Offset.Zero

                        if (startHit is PlanHit.Vertex) {
                            val pts = if (startHit.isPlane) {
                                gDraw.regions.firstOrNull { it.id == startHit.id && !it.locked }?.outline.orEmpty()
                            } else {
                                gDraw.bands.firstOrNull { it.id == startHit.id && !it.locked }?.path.orEmpty()
                            }
                            if (startHit.idx in pts.indices) {
                                dragVertex = startHit
                                vertexPts = pts
                                val vpx = startCam.toPx(pts[startHit.idx].first, pts[startHit.idx].second, w, h)
                                vertexGrab = Offset(vpx.first.toFloat() - downPos.x, vpx.second.toFloat() - downPos.y)
                            }
                        }

                        try {
                            while (true) {
                                val ev = awaitPointerEvent()
                                val active = ev.changes.filter { it.pressed }
                                if (active.isEmpty()) {
                                    liftedNormally = ev.changes.any { it.changedToUp() }
                                    break
                                }

                                if (active.size >= 2) {
                                    wasMulti = true
                                    dragVertex = null
                                    latestVertexPts = null
                                    dragPreview = null
                                    dragWorld = null
                                    dragScreen = null
                                    cameraIsFitted = false
                                    val p0 = toDp(active[0].position)
                                    val p1 = toDp(active[1].position)
                                    val centroid = Offset((p0.x + p1.x) * 0.5f, (p0.y + p1.y) * 0.5f)
                                    val span = hypot((p1.x - p0.x).toDouble(), (p1.y - p0.y).toDouble()).toFloat()
                                    if (prevSpan > 2f && span > 2f) {
                                        val dc = centroid - prevCentroid
                                        val factor = (span / prevSpan).toDouble().coerceIn(0.6, 1.8)
                                        multiCam = multiCam.pan(dc.x.toDouble(), dc.y.toDouble())
                                            .zoom(factor, centroid.x.toDouble(), centroid.y.toDouble(), w, h)
                                        cam = multiCam
                                    }
                                    prevSpan = span
                                    prevCentroid = centroid
                                    active.forEach { it.consume() }
                                } else {
                                    prevSpan = 0f
                                    val ch = active[0]
                                    val curDp = toDp(ch.position)
                                    val deltaDp = toDp(ch.position - ch.previousPosition)
                                    val dv = dragVertex
                                    if (dv != null && !wasMulti) {
                                        if (!moved && hypot((curDp.x - downPos.x).toDouble(), (curDp.y - downPos.y).toDouble()) > touchSlopDp) {
                                            moved = true
                                            selectedId = dv.id
                                            selectedVertex = dv.id to dv.idx
                                        }
                                        if (moved) {
                                            ch.consume()
                                            val targetDp = curDp + vertexGrab
                                            val (wx, wy) = startCam.toWorld(targetDp.x.toDouble(), targetDp.y.toDouble(), w, h)
                                            val snapped = PlanFloorplannerModel.round2(wx) to PlanFloorplannerModel.round2(wy)
                                            dragWorld = snapped
                                            dragScreen = targetDp
                                            val nextPts = vertexPts.toMutableList().apply { set(dv.idx, snapped) }
                                            latestVertexPts = nextPts
                                            val patch = if (dv.isPlane) PlanOptimisticPatch(outline = nextPts) else PlanOptimisticPatch(path = nextPts)
                                            dragPreview = dv.id to patch
                                        }
                                    } else if (!wasMulti) {
                                        totalPan += deltaDp
                                        if (!moved && hypot((curDp.x - downPos.x).toDouble(), (curDp.y - downPos.y).toDouble()) > touchSlopDp) {
                                            moved = true
                                            cameraIsFitted = false
                                        }
                                        if (moved) {
                                            ch.consume()
                                            cam = startCam.pan(totalPan.x.toDouble(), totalPan.y.toDouble())
                                        }
                                    }
                                }
                            }

                            if (liftedNormally && !moved && !wasMulti) {
                                val now = System.currentTimeMillis()
                                if (now - lastTapAt < 280L && hypot((downPos.x - lastTapPos.x).toDouble(), (downPos.y - lastTapPos.y).toDouble()) < touchSlopDp * 4) {
                                    lastTapAt = 0L
                                    fitCameraNow()
                                } else {
                                    lastTapAt = now
                                    lastTapPos = downPos
                                    when (startHit) {
                                        is PlanHit.Vertex -> {
                                            selectedId = startHit.id
                                            selectedVertex = (startHit.id to startHit.idx)
                                        }
                                        is PlanHit.Midpoint -> {
                                            if (startHit.isPlane) {
                                                gDraw.regions.firstOrNull { it.id == startHit.id && !it.locked }?.let { r ->
                                                    val nxt = r.outline[(startHit.segIdx + 1) % r.outline.size]
                                                    val mid = ((r.outline[startHit.segIdx].first + nxt.first) * 0.5) to ((r.outline[startHit.segIdx].second + nxt.second) * 0.5)
                                                    val updated = PlanFloorplannerModel.insertVertex(r.outline, startHit.segIdx, mid, true)
                                                    enqueueEdit(pro, r.id, PlanOptimisticPatch(outline = updated), PlanOptimisticPatch(outline = r.outline), gBaseRev, onlyGeom = "outline")
                                                    selectedId = r.id
                                                    selectedVertex = r.id to (startHit.segIdx + 1)
                                                }
                                            } else {
                                                gDraw.bands.firstOrNull { it.id == startHit.id && !it.locked }?.let { b ->
                                                    val mid = ((b.path[startHit.segIdx].first + b.path[startHit.segIdx + 1].first) * 0.5) to ((b.path[startHit.segIdx].second + b.path[startHit.segIdx + 1].second) * 0.5)
                                                    val updated = PlanFloorplannerModel.insertVertex(b.path, startHit.segIdx, mid, false)
                                                    enqueueEdit(pro, b.id, PlanOptimisticPatch(path = updated), PlanOptimisticPatch(path = b.path), gBaseRev, onlyGeom = "path")
                                                    selectedId = b.id
                                                    selectedVertex = b.id to (startHit.segIdx + 1)
                                                }
                                            }
                                        }
                                        is PlanHit.Band, is PlanHit.Region -> {
                                            val id = startHit.id
                                            val wasSame = gSel == id
                                            selectedId = id
                                            selectedVertex = null
                                            if (wasSame) inspectorOpen = true
                                            scope.launch { runCatching { DataClient.planFocus(pro, id) } }
                                        }
                                        null -> {
                                            selectedId = ""
                                            selectedVertex = null
                                        }
                                    }
                                }
                            } else if (liftedNormally && moved && !wasMulti) {
                                val dv = dragVertex
                                val draggedPts = latestVertexPts
                                if (dv != null && draggedPts != null) {
                                    dragPreview = null
                                    val prevPatch = if (dv.isPlane) PlanOptimisticPatch(outline = vertexPts) else PlanOptimisticPatch(path = vertexPts)
                                    val nextPatch = if (dv.isPlane) PlanOptimisticPatch(outline = draggedPts) else PlanOptimisticPatch(path = draggedPts)
                                    enqueueEdit(pro, dv.id, nextPatch, prevPatch, gBaseRev, onlyGeom = if (dv.isPlane) "outline" else "path")
                                } else {
                                    val elapsed = System.currentTimeMillis() - downTime
                                    val fitCam = PlanCamera.fit(gBoard.bbox, w, h, 22.0)
                                    if (elapsed < 340L && abs(startCam.scale - fitCam.scale) / max(fitCam.scale, 0.01) < 0.08 && abs(totalPan.x) > 55f && abs(totalPan.x) > abs(totalPan.y) * 1.6f && gLayers.size > 1) {
                                        val idx = gLayers.indexOfFirst { it.layer == curLayer }
                                        if (idx >= 0) {
                                            val step = if (totalPan.x < 0) 1 else -1
                                            activeLayer = gLayers[(idx + step + gLayers.size) % gLayers.size].layer
                                            selectedVertex = null
                                            cameraIsFitted = true
                                            cam = fitCam
                                        }
                                    }
                                }
                            }
                        } finally {
                            dragPreview = null
                            dragWorld = null
                            dragScreen = null
                        }
                    }
                }
        ) {
            Canvas(Modifier.fillMaxSize()) {
                val w = size.width.toDouble() / dpScale
                val h = size.height.toDouble() / dpScale
                val pt = { wx: Double, wy: Double ->
                    val (px, py) = activeCam.toPx(wx, wy, w, h)
                    Offset((px * dpScale).toFloat(), (py * dpScale).toFloat())
                }

                drawRect(Color(PCB_SUBSTRATE))

                // 2. Board interior
                val polys = board.outline.filter { it.size >= 3 }
                if (polys.isNotEmpty()) {
                    val p = Path().apply {
                        fillType = PathFillType.EvenOdd
                        for (poly in polys) {
                            val p0 = pt(poly[0].first, poly[0].second)
                            moveTo(p0.x, p0.y)
                            for (i in 1 until poly.size) {
                                val pi = pt(poly[i].first, poly[i].second)
                                lineTo(pi.x, pi.y)
                            }
                            close()
                        }
                    }
                    drawPath(p, Color(PCB_BOARD_FILL))
                } else {
                    val p0 = pt(board.bbox[0], board.bbox[1])
                    val p1 = pt(board.bbox[2], board.bbox[3])
                    drawRect(Color(PCB_BOARD_FILL), Offset(min(p0.x, p1.x), min(p0.y, p1.y)), Size(abs(p1.x - p0.x), abs(p1.y - p0.y)))
                }

                // 3. Power plane pours + in-pour rail labels
                for (r in draw.regions) {
                    if (r.layer != curLayer || r.outline.size < 3) continue
                    val sel = selectedId == r.id
                    val colRgb = PlanFloorplannerModel.railColorRgb(r.net.ifEmpty { r.id }, r.color)
                    val (fa, oa, bad) = when (r.state) {
                        "approved", "routing" -> Triple(if (sel) 0.42f else 0.30f, 0.84f, false)
                        "edited" -> Triple(if (sel) 0.45f else 0.34f, 0.90f, false)
                        "routed", "verified" -> Triple(0.17f, 0.46f, false)
                        "rejected", "failed" -> Triple(0.09f, 0.24f, false)
                        "blocked" -> Triple(0.35f, 0.95f, true)
                        else -> Triple(if (sel) 0.38f else 0.22f, 0.69f, false)
                    }
                    val fillP = Path().apply {
                        fillType = PathFillType.EvenOdd
                        val s0 = pt(r.outline[0].first, r.outline[0].second)
                        moveTo(s0.x, s0.y)
                        for (i in 1 until r.outline.size) {
                            val si = pt(r.outline[i].first, r.outline[i].second)
                            lineTo(si.x, si.y)
                        }
                        close()
                        for (hole in r.holes) {
                            if (hole.size < 3) continue
                            val h0 = pt(hole[0].first, hole[0].second)
                            moveTo(h0.x, h0.y)
                            for (i in 1 until hole.size) {
                                val hi = pt(hole[i].first, hole[i].second)
                                lineTo(hi.x, hi.y)
                            }
                            close()
                        }
                    }
                    drawPath(fillP, rgbColor(colRgb, fa))
                    val edgeP = Path().apply {
                        val s0 = pt(r.outline[0].first, r.outline[0].second)
                        moveTo(s0.x, s0.y)
                        for (i in 1 until r.outline.size) {
                            val si = pt(r.outline[i].first, r.outline[i].second)
                            lineTo(si.x, si.y)
                        }
                        close()
                    }
                    drawPath(edgeP, if (bad) Color(PCB_BAD_RED) else if (sel) Color.White else rgbColor(colRgb, oa), style = Stroke((if (sel || bad) 2f else 1.25f) * dpScale, join = StrokeJoin.Round))

                    if (r.net.isNotEmpty()) {
                        var sx0 = Float.POSITIVE_INFINITY; var sy0 = Float.POSITIVE_INFINITY
                        var sx1 = Float.NEGATIVE_INFINITY; var sy1 = Float.NEGATIVE_INFINITY
                        for (v in r.outline) {
                            val q = pt(v.first, v.second)
                            sx0 = min(sx0, q.x); sy0 = min(sy0, q.y)
                            sx1 = max(sx1, q.x); sy1 = max(sy1, q.y)
                        }
                        if (abs(sx1 - sx0) >= 38f * dpScale && abs(sy1 - sy0) >= 18f * dpScale) {
                            PlanFloorplannerModel.pourLabelPoint(r.outline, r.holes)?.let { lp ->
                                val lpx = pt(lp.first, lp.second)
                                drawIntoCanvas { cv ->
                                    val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
                                        color = ((if (sel) 232 else 170) shl 24) or (colRgb and 0x00FFFFFF)
                                        typeface = Typeface.create(Typeface.MONOSPACE, Typeface.BOLD)
                                        textSize = 10f * dpScale
                                        textAlign = Paint.Align.CENTER
                                    }
                                    cv.nativeCanvas.drawText(r.net, lpx.x, lpx.y + 3.5f * dpScale, paint)
                                }
                            }
                        }
                    }
                }

                // 4. Crisp Edge.Cuts outline
                if (polys.isNotEmpty()) {
                    val ep = Path().apply {
                        for (poly in polys) {
                            val e0 = pt(poly[0].first, poly[0].second)
                            moveTo(e0.x, e0.y)
                            for (i in 1 until poly.size) {
                                val ei = pt(poly[i].first, poly[i].second)
                                lineTo(ei.x, ei.y)
                            }
                            close()
                        }
                    }
                    drawPath(ep, Color(PCB_EDGE), style = Stroke(1.25f * dpScale, cap = StrokeCap.Round, join = StrokeJoin.Round))
                } else {
                    val p0 = pt(board.bbox[0], board.bbox[1])
                    val p1 = pt(board.bbox[2], board.bbox[3])
                    drawRect(Color(PCB_EDGE), Offset(min(p0.x, p1.x), min(p0.y, p1.y)), Size(abs(p1.x - p0.x), abs(p1.y - p0.y)), style = Stroke(1.25f * dpScale))
                }

                // 5. Component courtyards
                val dashEff = PathEffect.dashPathEffect(floatArrayOf(4f * dpScale, 3f * dpScale), 0f)
                for (p in board.parts) {
                    if (p.b.size != 4) continue
                    val p0 = pt(p.b[0], p.b[1]); val p1 = pt(p.b[2], p.b[3])
                    val tl = Offset(min(p0.x, p1.x), min(p0.y, p1.y))
                    val sz = Size(abs(p1.x - p0.x), abs(p1.y - p0.y))
                    drawRect(
                        if (p.o) Color(PCB_OVERHANG) else Color(PCB_PART),
                        tl, sz,
                        style = if (p.o) Stroke(0.95f * dpScale, pathEffect = dashEff) else Stroke(0.85f * dpScale),
                    )
                }

                // 6. Refdes labels avoiding corridor pills
                val layerBands = draw.bands.filter { it.layer == curLayer }
                val pills = PlanFloorplannerModel.placeCorridorPills(layerBands, activeCam, w, h, selectedId)
                val pillBoxes = pills.map { it.rect.pad(1.0) }
                val pinnedRefs = layerBands.flatMap { it.from + it.to }
                val refPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
                    color = PCB_REFDES.toInt()
                    typeface = Typeface.MONOSPACE
                    textSize = 9.5f * dpScale
                    textAlign = Paint.Align.CENTER
                }
                drawIntoCanvas { cv ->
                    for (lbl in PlanFloorplannerModel.layoutLabels(board.parts, activeCam, w, h, pinnedRefs, pillBoxes)) {
                        cv.nativeCanvas.drawText(lbl.text, (lbl.cx * dpScale).toFloat(), (lbl.cy * dpScale).toFloat() + 3.2f * dpScale, refPaint)
                    }
                }

                // 7. Faint signal ratsnest guides
                for (b in layerBands) {
                    if (b.guides.isEmpty()) continue
                    val act = selectedId == b.id
                    val col = PlanFloorplannerModel.busColorRgb(b.name.ifEmpty { b.id }, b.color)
                    for (seg in b.guides) {
                        if (seg.size != 4) continue
                        drawLine(rgbColor(col, if (act) 0.38f else 0.17f), pt(seg[0], seg[1]), pt(seg[2], seg[3]), strokeWidth = (if (act) 1.0f else 0.7f) * dpScale)
                    }
                }

                // 8 & 9. Multi-pass signal corridors & diamond vias
                for (b in layerBands) {
                    if (b.path.size < 2) continue
                    val sel = selectedId == b.id
                    val bad = b.state == "blocked"
                    val (ga, ba, ca) = when (b.state) {
                        "approved", "routing" -> Triple(if (sel) 0.26f else 0.16f, if (sel) 0.63f else 0.51f, if (sel) 1f else 0.91f)
                        "edited" -> Triple(if (sel) 0.30f else 0.20f, if (sel) 0.68f else 0.56f, if (sel) 1f else 0.95f)
                        "routed", "verified" -> Triple(0.08f, 0.26f, 0.59f)
                        "rejected", "failed" -> Triple(0.05f, 0.13f, 0.27f)
                        "blocked" -> Triple(0.28f, 0.58f, 0.95f)
                        else -> Triple(if (sel) 0.24f else 0.14f, if (sel) 0.57f else 0.45f, if (sel) 1f else 0.85f)
                    }
                    val col = PlanFloorplannerModel.busColorRgb(b.name.ifEmpty { b.id }, b.color)
                    val wpx = (PlanFloorplannerModel.bandStrokePx(b.width, activeCam.scale) * dpScale).toFloat()
                    val polyP = Path().apply {
                        val s0 = pt(b.path[0].first, b.path[0].second)
                        moveTo(s0.x, s0.y)
                        for (i in 1 until b.path.size) {
                            val si = pt(b.path[i].first, b.path[i].second)
                            lineTo(si.x, si.y)
                        }
                    }
                    drawPath(polyP, rgbColor(col, ga), style = Stroke(wpx + 6f * dpScale, cap = StrokeCap.Round, join = StrokeJoin.Round))
                    if (bad || sel) {
                        drawPath(polyP, if (bad) Color(PCB_BAD_RED) else Color.White.copy(alpha = 0.78f), style = Stroke(wpx + 2.5f * dpScale, cap = StrokeCap.Round, join = StrokeJoin.Round))
                    }
                    drawPath(polyP, rgbColor(col, ba), style = Stroke(wpx, cap = StrokeCap.Round, join = StrokeJoin.Round))
                    drawPath(polyP, rgbColor(col, ca), style = Stroke((wpx * 0.34f).coerceIn(1.8f * dpScale, 5f * dpScale), cap = StrokeCap.Round, join = StrokeJoin.Round))

                    for (v in b.vias) {
                        val q = pt(v.first, v.second); val r = 4.2f * dpScale
                        val dp = Path().apply {
                            moveTo(q.x, q.y - r); lineTo(q.x + r, q.y); lineTo(q.x, q.y + r); lineTo(q.x - r, q.y); close()
                        }
                        drawPath(dp, Color(PCB_SUBSTRATE), style = Fill)
                        drawPath(dp, if (sel) Color.White else rgbColor(col), style = Stroke(1.5f * dpScale))
                    }
                }

                // 10. Corridor pills
                val pillTxtPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
                    color = PCB_BADGE_FG.toInt()
                    typeface = Typeface.create(Typeface.MONOSPACE, Typeface.BOLD)
                    textSize = 10f * dpScale
                    textAlign = Paint.Align.CENTER
                }
                for (pill in pills) {
                    val pw = (pill.pw * dpScale).toFloat()
                    val ph = (pill.ph * dpScale).toFloat()
                    val cxPx = (pill.cx * dpScale).toFloat()
                    val cyPx = (pill.cy * dpScale).toFloat()
                    val tl = Offset(cxPx - pw * 0.5f, cyPx - ph * 0.5f)
                    val sz = Size(pw, ph)
                    drawRect(Color(PCB_BADGE_BG), tl, sz)
                    drawRect(if (pill.selected) Color.White else rgbColor(pill.colorRgb), tl, sz, style = Stroke(1.2f * dpScale))
                    drawIntoCanvas { cv ->
                        cv.nativeCanvas.drawText(pill.text, cxPx, cyPx + 3.5f * dpScale, pillTxtPaint)
                    }
                }

                // 11. Touch-friendly vertex & + midpoint handles
                if (selectedId.isNotEmpty()) {
                    val paintHandles = { pts: List<Pair<Double, Double>>, closed: Boolean ->
                        val segs = if (closed) pts.size else max(0, pts.size - 1)
                        val plusPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
                            color = android.graphics.Color.WHITE
                            typeface = Typeface.create(Typeface.MONOSPACE, Typeface.BOLD)
                            textSize = 11f * dpScale
                            textAlign = Paint.Align.CENTER
                        }
                        for (i in 0 until segs) {
                            val a = pts[i]; val b = pts[(i + 1) % pts.size]
                            val mp = pt((a.first + b.first) * 0.5, (a.second + b.second) * 0.5)
                            val rSz = 20f * dpScale
                            drawRect(Color(PCB_BOARD_FILL), Offset(mp.x - rSz * 0.5f, mp.y - rSz * 0.5f), Size(rSz, rSz))
                            drawRect(Color.White.copy(alpha = 0.7f), Offset(mp.x - rSz * 0.5f, mp.y - rSz * 0.5f), Size(rSz, rSz), style = Stroke(1.2f * dpScale))
                            drawIntoCanvas { cv -> cv.nativeCanvas.drawText("+", mp.x, mp.y + 4f * dpScale, plusPaint) }
                        }
                        for (i in pts.indices) {
                            val q = pt(pts[i].first, pts[i].second)
                            val isSelV = selectedVertex?.first == selectedId && selectedVertex?.second == i
                            val vSz = (if (isSelV) 18f else 14f) * dpScale
                            drawRect(if (isSelV) Color(PCB_OVERHANG) else Color.White, Offset(q.x - vSz * 0.5f, q.y - vSz * 0.5f), Size(vSz, vSz))
                            drawRect(Color(PCB_SUBSTRATE), Offset(q.x - vSz * 0.5f, q.y - vSz * 0.5f), Size(vSz, vSz), style = Stroke(1.8f * dpScale))
                        }
                    }
                    draw.bands.firstOrNull { it.id == selectedId && it.layer == curLayer && !it.locked }?.let { paintHandles(it.path, false) }
                    draw.regions.firstOrNull { it.id == selectedId && it.layer == curLayer && !it.locked }?.let { paintHandles(it.outline, true) }
                }
            }

            // Finger-lift floating reticle + coordinate pill 44dp above dragged waypoint
            val wp = dragWorld
            val sp = dragScreen
            if (wp != null && sp != null) {
                val coordText = Msg.mobile_plan_coord(PlanFloorplannerModel.fmt2(wp.first), PlanFloorplannerModel.fmt2(wp.second))
                val offX = (sp.x - 56f).coerceIn(8f, (canvasDpW.toFloat() - 130f).coerceAtLeast(8f)).dp
                val offY = (sp.y - 44f - 20f).coerceAtLeast(6f).dp
                Box(
                    Modifier
                        .offset(x = offX, y = offY)
                        .background(Color(PCB_BADGE_BG))
                        .border(BorderStroke(1.dp, dcs.primary))
                        .padding(horizontal = 8.dp, vertical = 4.dp),
                ) {
                    DMono(coordText, Color(PCB_BADGE_FG), 11, bold = true)
                }
            }

            if (selectedId.isEmpty()) {
                Box(
                    Modifier.align(Alignment.TopStart).padding(6.dp).background(Color(PCB_SUBSTRATE).copy(alpha = 0.85f)).padding(horizontal = 6.dp, vertical = 3.dp),
                ) {
                    DMono(Msg.mobile_plan_touchhint, dfaint, 10)
                }
            }
        }

        DLine()
        if (selectedId.isNotEmpty()) {
            val found = findPlanItem(selectedId, panel.layers)
            val name = found?.second?.name?.ifEmpty { selectedId } ?: selectedId
            val col = if (found?.second?.kind == "plane") PlanFloorplannerModel.railColorRgb(name, found.second.color) else PlanFloorplannerModel.busColorRgb(name, found?.second?.color.orEmpty())
            val isLocked = found?.second?.locked == true
            val sv = selectedVertex
            val remSv = if (!isLocked && sv != null && sv.first == selectedId && (
                draw.bands.firstOrNull { it.id == sv.first }?.let { it.path.size > 2 } == true ||
                    draw.regions.firstOrNull { it.id == sv.first }?.let { it.outline.size > 3 } == true
                )) sv else null
            Row(
                Modifier.fillMaxWidth().background(dcs.surfaceVariant).horizontalScroll(rememberScrollState()).padding(horizontal = 10.dp, vertical = 6.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                Box(Modifier.size(10.dp).background(rgbColor(col)))
                DMono(name, dcs.onSurface, 11, bold = true)
                DBtn(Msg.mobile_plan_details, false, small = true, modifier = Modifier.testTag("plan.details")) { inspectorOpen = true }
                if (remSv != null) {
                    DBtn(Msg.mobile_plan_removepoint, false, small = true, danger = true) {
                        val b = draw.bands.firstOrNull { it.id == remSv.first }
                        val r = draw.regions.firstOrNull { it.id == remSv.first }
                        if (b != null) {
                            PlanFloorplannerModel.removeVertex(b.path, remSv.second, false)?.let { pts ->
                                selectedVertex = null
                                enqueueEdit(pro, b.id, PlanOptimisticPatch(path = pts), PlanOptimisticPatch(path = b.path), panel.revision, onlyGeom = "path")
                            }
                        } else if (r != null) {
                            PlanFloorplannerModel.removeVertex(r.outline, remSv.second, true)?.let { pts ->
                                selectedVertex = null
                                enqueueEdit(pro, r.id, PlanOptimisticPatch(outline = pts), PlanOptimisticPatch(outline = r.outline), panel.revision, onlyGeom = "outline")
                            }
                        }
                    }
                }
                if (PlanFloorplannerModel.hasUndo(undoStack, pro, selectedId)) {
                    DBtn(Msg.gpui_mgmt_bom_undo, false, small = true) {
                        val (prev, nextStack) = PlanFloorplannerModel.popUndo(undoStack, pro, selectedId)
                        undoStack = nextStack
                        if (prev != null) {
                            enqueueEdit(pro, prev.id, prev.patch, undoPrev = null, baseRev = panel.revision, onlyGeom = prev.onlyGeom)
                            prev.patch.layer?.takeIf { it.isNotEmpty() }?.let { mv -> activeLayer = mv }
                        }
                    }
                }
                if (pendingEdits.any { it.pro == pro && it.itemId == selectedId }) {
                    DBtn(Msg.mobile_plan_reset, false, small = true) {
                        pendingEdits = pendingEdits.filterNot { it.pro == pro && it.itemId == selectedId }
                        undoStack = undoStack.filterNot { it.pro == pro && it.id == selectedId }
                        selectedVertex = null
                        scope.launch { refreshAuthoritativePlan(pro) }
                    }
                }
                DBtn(Msg.tour_done, true, small = true) { selectedId = ""; selectedVertex = null }
            }
        } else {
            val items = layerObj?.items.orEmpty()
            if (items.isEmpty()) {
                DMono(Msg.gpui_mgmt_plan_noitems, dfaint, 11, Modifier.fillMaxWidth().background(dcs.surfaceVariant).padding(horizontal = 10.dp, vertical = 8.dp))
            } else {
                Row(
                    Modifier.fillMaxWidth().background(dcs.surfaceVariant).horizontalScroll(rememberScrollState()).padding(horizontal = 10.dp, vertical = 6.dp),
                    horizontalArrangement = Arrangement.spacedBy(6.dp),
                ) {
                    items.forEach { it ->
                        val col = if (it.kind == "plane") PlanFloorplannerModel.railColorRgb(it.name, it.color) else PlanFloorplannerModel.busColorRgb(it.name, it.color)
                        val stC = when (it.state) {
                            "blocked", "failed" -> dcs.error
                            "approved" -> dcs.primary
                            "routed", "verified" -> dok
                            "edited" -> dwarn
                            else -> dcs.outlineVariant
                        }
                        Row(
                            Modifier.height(30.dp).background(dcs.surface).border(BorderStroke(1.dp, stC)).clickable {
                                selectedId = it.id
                                selectedVertex = null
                                inspectorOpen = true
                                scope.launch { runCatching { DataClient.planFocus(pro, it.id) } }
                            }.padding(horizontal = 8.dp).testTag("plan.pick.${it.id}"),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(5.dp),
                        ) {
                            Box(Modifier.size(8.dp).background(rgbColor(col)))
                            DMono(it.name.ifEmpty { it.id }, dcs.onSurface, 11)
                            if (it.nets > 0) DMono(it.nets.toString(), dcs.onSurfaceVariant, 10)
                        }
                    }
                }
            }
        }
    }

    if (inspectorOpen) {
        val found = findPlanItem(selectedId, panel.layers)
        if (found != null) {
            PlanItemInspectorSheet(
                item = found.second,
                curLayer = found.first,
                copper = panel.copperLayers,
                allLayers = layers,
                onApprove = { ok -> scope.launch { runCatching { DataClient.planApprove(pro, "", listOf(found.second.id), ok) }.onFailure { errMsg = it.message ?: Msg.android_failed_generic } } },
                onSteer = { inspectorOpen = false },
                onCommit = { patch ->
                    val prev = captureUndoSnapshot(found.second.id, patch)
                    enqueueEdit(pro, found.second.id, patch, prev, panel.revision)
                    patch.layer?.let { mv -> if (mv != found.first) activeLayer = mv }
                },
                onNote = { txt ->
                    val op = JSONObject().put("op", "note").put("target", found.second.id).put("text", txt)
                    val sendBase = max(panel.revision, ackedRev)
                    scope.launch { runCatching { DataClient.planUpdate(pro, listOf(op), sendBase) }.onFailure { errMsg = it.message ?: Msg.android_failed_generic } }
                },
                onDismiss = { inspectorOpen = false },
            )
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun PlanItemInspectorSheet(
    item: PlanItem,
    curLayer: String,
    copper: List<PlanCopperLayer>,
    allLayers: List<PlanLayer>,
    onApprove: (Boolean) -> Unit,
    onSteer: () -> Unit,
    onCommit: (PlanOptimisticPatch) -> Unit,
    onNote: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    var note by remember(item.id) { mutableStateOf("") }
    val col = if (item.kind == "plane") PlanFloorplannerModel.railColorRgb(item.name, item.color) else PlanFloorplannerModel.busColorRgb(item.name, item.color)
    val layerTitle = PlanFloorplannerModel.layerDisplayTitle(curLayer, copper, allLayers)
    val widthMm = item.width ?: 1.5
    val moveOpts = if (copper.isEmpty()) allLayers.map { it.layer } else copper.map { it.layer }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        shape = RectangleShape,
        containerColor = dcs.surfaceVariant,
        dragHandle = null,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
    ) {
        Column(Modifier.fillMaxWidth().padding(14.dp).navigationBarsPadding(), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Box(Modifier.size(12.dp).background(rgbColor(col)))
                DMono(item.name.ifEmpty { item.id }, dcs.onSurface, 14, bold = true)
                DMono(layerTitle, dcs.onSurfaceVariant, 12)
                if (item.edited || item.state == "edited") {
                    DMono(Msg.gpui_mgmt_plan_edited, dwarn, 10, Modifier.border(BorderStroke(1.dp, dwarn)).padding(horizontal = 4.dp, vertical = 1.dp))
                }
                if (item.locked) {
                    DMono(Msg.gpui_mgmt_plan_locked, dcs.onSurfaceVariant, 10, Modifier.border(BorderStroke(1.dp, dcs.outlineVariant)).padding(horizontal = 4.dp, vertical = 1.dp))
                }
                Spacer(Modifier.weight(1f))
                Icon(Icons.Default.Close, Msg.common_close, tint = dcs.onSurfaceVariant, modifier = Modifier.size(20.dp).clickable(onClick = onDismiss))
            }

            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                if (item.kind == "corridor") DMono(Msg.gpui_mgmt_plan_nets(item.nets), dcs.onSurfaceVariant, 12)
                else if (item.reference.isNotEmpty()) DMono(Msg.gpui_mgmt_plan_reference(item.reference), dcs.onSurfaceVariant, 12)
                if (item.impedance.isNotEmpty()) DMono(item.impedance, dcs.onSurfaceVariant, 12)
            }
            if (item.problemText.isNotEmpty()) DMono(item.problemText, dcs.error, 11, lines = 3)

            Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                if (item.state == "proposed" || item.state == "edited") {
                    DBtn(Msg.android_hub_machines_approve, true, small = true) { onApprove(true); onDismiss() }
                    DBtn(Msg.gpui_mgmt_plan_reject, false, small = true, danger = true) { onApprove(false); onDismiss() }
                }
                DBtn(if (item.locked) Msg.gpui_mgmt_plan_unlock else Msg.gpui_mgmt_plan_lock, false, small = true) {
                    onCommit(PlanOptimisticPatch(locked = !item.locked))
                }
                if (!item.locked) {
                    DBtn(Msg.mobile_plan_steer, false, small = true, onClick = onSteer)
                }
            }

            if (!item.locked && item.kind == "corridor") {
                DLine()
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    DMono(Msg.gpui_mgmt_plan_width, dcs.onSurfaceVariant, 12)
                    DBtn("−", false, small = true) {
                        val nw = PlanFloorplannerModel.round2(max(0.15, widthMm - 0.25))
                        onCommit(PlanOptimisticPatch(width = nw))
                    }
                    DMono(Msg.mobile_plan_widthmm(PlanFloorplannerModel.fmt2(widthMm)), dcs.onSurface, 13, bold = true)
                    DBtn("+", false, small = true) {
                        val nw = PlanFloorplannerModel.round2(min(25.0, widthMm + 0.25))
                        onCommit(PlanOptimisticPatch(width = nw))
                    }
                }
            }

            if (!item.locked && moveOpts.isNotEmpty()) {
                DLine()
                DMono(Msg.gpui_mgmt_plan_layer_move, dfaint, 11)
                Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    moveOpts.forEach { ml ->
                        val title = PlanFloorplannerModel.layerDisplayTitle(ml, copper, allLayers)
                        DBtn(title, ml == curLayer, small = true) {
                            if (ml != curLayer) {
                                onCommit(PlanOptimisticPatch(layer = ml))
                                onDismiss()
                            }
                        }
                    }
                }
            }

            DLine()
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                Box(Modifier.weight(1f)) { DField(note, { note = it }, Msg.gpui_mgmt_plan_note_placeholder) }
                DBtn(Msg.gpui_mgmt_plan_addnote, true, note.isNotBlank(), small = true) {
                    val t = note.trim()
                    if (t.isNotEmpty()) { note = ""; onNote(t); onDismiss() }
                }
            }
        }
    }
}

@Composable
private fun PlanMiniChip(label: String, active: Boolean, onClick: () -> Unit) {
    Box(
        Modifier.height(26.dp).background(if (active) dcs.primary else dcs.surface).border(BorderStroke(1.dp, if (active) dcs.primary else dcs.outlineVariant))
            .clickable(onClick = onClick).padding(horizontal = 8.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            label,
            color = if (active) dcs.onPrimary else dcs.onSurfaceVariant,
            fontFamily = BpMono,
            fontSize = 10.sp,
            fontWeight = if (active) FontWeight.Bold else FontWeight.Normal,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

@Composable
private fun PlanProgressBar(panel: PlanPanel) {
    val tot = max(1, panel.total).toFloat()
    Row(Modifier.fillMaxWidth().height(3.dp).background(dcs.outlineVariant)) {
        if (panel.done > 0) Box(Modifier.fillMaxHeight().weight(panel.done / tot).background(dok))
        if (panel.approved > 0) Box(Modifier.fillMaxHeight().weight(panel.approved / tot).background(dinput))
        if (panel.awaiting > 0) Box(Modifier.fillMaxHeight().weight(panel.awaiting / tot).background(dwarn))
        if (panel.blocked > 0) Box(Modifier.fillMaxHeight().weight(panel.blocked / tot).background(dcs.error))
        val rem = (panel.total - panel.done - panel.approved - panel.awaiting - panel.blocked).coerceAtLeast(0)
        if (rem > 0) Spacer(Modifier.weight(rem / tot))
    }
}

@Composable
private fun planStatusMeta(d: PlanPanel): Pair<String, Color> = when {
    d.status == "blocked" || d.blocked > 0 -> Msg.gpui_mgmt_plan_st_blocked to dcs.error
    d.status == "routing" || d.routing -> Msg.gpui_mgmt_plan_st_routing to dcs.primary
    d.status == "awaiting" || d.awaiting > 0 -> Msg.gpui_mgmt_plan_st_awaiting to dwarn
    d.status == "verified" -> Msg.gpui_mgmt_plan_st_verified to dok
    d.status == "routed" -> Msg.gpui_mgmt_plan_st_routed to dok
    d.status == "approved" -> Msg.gpui_mgmt_plan_st_approved to dcs.primary
    d.status == "partial" -> Msg.gpui_mgmt_plan_st_partial to dcs.onSurfaceVariant
    d.status == "rejected" -> Msg.gpui_mgmt_plan_st_rejected to dfaint
    else -> Msg.gpui_mgmt_plan_st_empty to dfaint
}

private fun findPlanItem(id: String, layers: List<PlanLayer>): Pair<String, PlanItem>? {
    for (l in layers) {
        l.items.firstOrNull { it.id == id }?.let { return l.layer to it }
    }
    return null
}
