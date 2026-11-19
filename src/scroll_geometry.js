function clientBounds(element) {
    const rect = element.getBoundingClientRect();
    const scaleX = element.offsetWidth ? rect.width / element.offsetWidth : 1;
    const scaleY = element.offsetHeight ? rect.height / element.offsetHeight : 1;
    const left = rect.left + element.clientLeft * scaleX;
    const top = rect.top + element.clientTop * scaleY;
    return {
        left, top,
        right: left + element.clientWidth * scaleX,
        bottom: top + element.clientHeight * scaleY,
    };
}

function clippingAxes(style) {
    const clips = value => /^(auto|scroll|hidden|clip|overlay)$/.test(value);
    const paint = /(^| )(paint|strict|content)( |$)/.test(style.contain);
    return [paint || clips(style.overflowX), paint || clips(style.overflowY)];
}

function clipBounds(bounds, clip, clipX, clipY) {
    return {
        left: clipX ? Math.max(bounds.left, clip.left) : bounds.left,
        right: clipX ? Math.min(bounds.right, clip.right) : bounds.right,
        top: clipY ? Math.max(bounds.top, clip.top) : bounds.top,
        bottom: clipY ? Math.min(bounds.bottom, clip.bottom) : bounds.bottom,
    };
}
