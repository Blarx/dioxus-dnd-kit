const MAX_EDGE_SIZE = 48; // CSS pixels
const MAX_SCROLL_SPEED = 900; // CSS pixels per second

function edgeStep(pointer, start, end, elapsed) {
    const edge = Math.min(MAX_EDGE_SIZE, (end - start) / 4);
    const distance = Math.min(pointer - start, end - pointer);
    if (distance < 0 || distance >= edge) return 0;
    const strength = 1 - distance / edge;
    const direction = pointer - start < end - pointer ? -1 : 1;
    return direction * Math.max(1, strength * MAX_SCROLL_SPEED * elapsed / 1000);
}

function scrollportsAt(hit) {
    const viewport = document.scrollingElement;
    const viewportStyle = getComputedStyle(document.documentElement);
    const bodyStyle = document.body && getComputedStyle(document.body);
    const port = (element, bounds, overflow, canScroll) => ({
        element, bounds,
        x: canScroll(overflow.overflowX) && element.scrollWidth > element.clientWidth,
        y: canScroll(overflow.overflowY) && element.scrollHeight > element.clientHeight,
    });
    const scrollable = value => /^(auto|scroll|overlay)$/.test(value);
    const viewportBounds = {
        left: 0, top: 0,
        right: document.documentElement.clientWidth,
        bottom: document.documentElement.clientHeight,
    };
    const ancestors = [];
    for (let element = hit; element; element = element.parentElement) {
        if (element === viewport || element === document.documentElement) continue;
        if (element === document.body && viewportStyle.overflow === "visible") continue;
        const style = getComputedStyle(element);
        const [clipX, clipY] = clippingAxes(style);
        if (clipX || clipY) ancestors.push({ element, style, clipX, clipY });
    }

    const ports = [];
    if (viewport) {
        const style = viewportStyle.overflow === "visible" && bodyStyle ? bodyStyle : viewportStyle;
        ports.push(port(viewport, viewportBounds, style, value => value === "visible" || scrollable(value)));
    }
    // Clip from the viewport inward, then try the innermost scroller first.
    let visible = viewportBounds;
    for (const { element, style, clipX, clipY } of ancestors.reverse()) {
        const bounds = clientBounds(element);
        const clipped = clipBounds(bounds, visible, true, true);
        ports.push(port(element, clipped, style, scrollable));
        visible = clipBounds(visible, bounds, clipX, clipY);
    }
    return ports.reverse();
}

function createAutoScroll(selector) {
    let root = null;
    let pointer = null;
    let frame = null;
    let previousTime = null;

    function tick(time) {
        if (!root.isConnected) {
            stop();
            return;
        }
        frame = requestAnimationFrame(tick);
        const elapsed = previousTime === null ? 16 : Math.min(time - previousTime, 34);
        previousTime = time;

        const [x, y] = pointer;
        const hit = document.elementFromPoint(x, y);
        if (!root.contains(hit)) return;

        let scrolledX = false, scrolledY = false;
        for (const { element, bounds, x: canScrollX, y: canScrollY } of scrollportsAt(hit)) {
            if (x < bounds.left || x >= bounds.right || y < bounds.top || y >= bounds.bottom) continue;
            const left = canScrollX && !scrolledX ? edgeStep(x, bounds.left, bounds.right, elapsed) : 0;
            const top = canScrollY && !scrolledY ? edgeStep(y, bounds.top, bounds.bottom, elapsed) : 0;
            if (!left && !top) continue;

            // A scroller at its limit leaves this axis available to its ancestors.
            const beforeX = element.scrollLeft, beforeY = element.scrollTop;
            element.scrollBy({ left, top, behavior: "instant" });
            scrolledX ||= element.scrollLeft !== beforeX;
            scrolledY ||= element.scrollTop !== beforeY;
            if (scrolledX && scrolledY) break;
        }
    }

    function stop() {
        if (frame !== null) cancelAnimationFrame(frame);
        frame = null;
        previousTime = null;
        pointer = null;
        root = null;
    }

    return {
        move(x, y) {
            pointer = [x, y];
            if (frame !== null) return;
            root = document.querySelector(selector);
            if (root) frame = requestAnimationFrame(tick);
        },
        stop,
    };
}
