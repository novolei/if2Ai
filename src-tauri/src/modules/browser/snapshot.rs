//! DOM AXTree snapshot script.
//!
//! Ported from openhanako `desktop/main.cjs::SNAPSHOT_SCRIPT`.
//! Changes from the original:
//!
//! - `data-hana-ref` → `data-if2ai-ref` (project-specific attribute)
//! - Return shape kept identical: `{ text, title, currentUrl }`
//! - Added SSRF protection: the caller (session.rs) should not navigate to
//!   `file://`, `javascript:`, or RFC-1918 addresses; this script only
//!   operates on already-loaded pages.
//!
//! The script is injected into every loaded page via `page.evaluate()`.
//! It annotates interactive DOM elements with sequential integer refs so the
//! LLM can reference them in `click`, `type`, and `select` commands.

/// JavaScript source for the AXTree snapshot.
///
/// Injected via `page.evaluate()`. Returns an object:
/// ```js
/// { text: string, title: string, currentUrl: string }
/// ```
///
/// `text` contains the human-readable AXTree representation with
/// `[N] role "label"` lines for each interactive element.
pub const SNAPSHOT_SCRIPT: &str = r#"(function() {
  var ref = 0;
  var MAX_TREE = 30000;

  // Clear previous refs from a prior snapshot run.
  document.querySelectorAll('[data-if2ai-ref]').forEach(function(el) {
    el.removeAttribute('data-if2ai-ref');
  });

  function isVisible(el) {
    if (!el.offsetParent && el.tagName !== 'BODY' && el.tagName !== 'HTML') return false;
    var s = window.getComputedStyle(el);
    return s.display !== 'none' && s.visibility !== 'hidden';
  }

  function isInteractive(el) {
    var t = el.tagName;
    if (['A','BUTTON','INPUT','TEXTAREA','SELECT','DETAILS','SUMMARY'].indexOf(t) !== -1) return true;
    var r = el.getAttribute('role');
    if (r && ['button','link','menuitem','tab','checkbox','radio','textbox',
               'combobox','listbox','option','switch','slider','treeitem'].indexOf(r) !== -1) return true;
    if (el.onclick || el.hasAttribute('onclick')) return true;
    if (el.contentEditable === 'true') return true;
    if (el.tabIndex > 0) return true;
    try {
      if (window.getComputedStyle(el).cursor === 'pointer' && !el.closest('a,button')) return true;
    } catch(e) {}
    return false;
  }

  function directText(el) {
    var t = '';
    for (var i = 0; i < el.childNodes.length; i++) {
      if (el.childNodes[i].nodeType === 3) t += el.childNodes[i].textContent;
    }
    return t.trim().replace(/\s+/g, ' ').slice(0, 80);
  }

  // Structure signature: direct child tag sequence, used to detect homogeneous siblings.
  function sig(el) {
    if (el.nodeType !== 1 || !isVisible(el)) return null;
    var tag = el.tagName;
    if (['SCRIPT','STYLE','NOSCRIPT','TEMPLATE','SVG'].indexOf(tag) !== -1) return null;
    var s = tag;
    for (var i = 0; i < el.children.length; i++) {
      var c = el.children[i];
      if (c.nodeType === 1 && isVisible(c) &&
          ['SCRIPT','STYLE','NOSCRIPT','TEMPLATE','SVG'].indexOf(c.tagName) === -1) {
        s += ',' + c.tagName;
      }
    }
    return s;
  }

  // Compact single-line format: link | button | text1 · text2
  function compact(el, depth) {
    var links = [], ctrls = [], texts = [];
    function collect(node) {
      if (node.nodeType !== 1 || !isVisible(node)) return;
      var tag = node.tagName;
      if (['SCRIPT','STYLE','NOSCRIPT','TEMPLATE','SVG'].indexOf(tag) !== -1) return;
      if (isInteractive(node)) {
        ref++;
        node.setAttribute('data-if2ai-ref', String(ref));
        var name = node.getAttribute('aria-label') || node.title || node.placeholder
          || (node.textContent || '').trim().replace(/\s+/g, ' ').slice(0, 60) || node.value || '';
        if (tag === 'A' || node.getAttribute('role') === 'link') {
          links.push('[' + ref + '] "' + name + '"');
        } else {
          ctrls.push('[' + ref + '] ' + name);
        }
        return; // Interactive element's subtree is captured via textContent; skip recursion.
      }
      var txt = directText(node);
      if (txt && txt.length > 2) texts.push(txt);
      for (var i = 0; i < node.children.length; i++) collect(node.children[i]);
    }
    collect(el);
    if (!links.length && !ctrls.length && !texts.length) return '';
    var pad = '';
    for (var i = 0; i < depth; i++) pad += '  ';
    var parts = links.concat(ctrls);
    var line = parts.join(' | ');
    if (texts.length) line += (line ? ' | ' : '') + texts.join(' \u00b7 ');
    return pad + line + '\n';
  }

  // Group traversal: ≥3 homogeneous siblings use compact(), others use walk().
  function walkChildren(el, depth) {
    var out = '';
    var children = [], sigs = [];
    for (var i = 0; i < el.children.length; i++) {
      children.push(el.children[i]);
      sigs.push(sig(el.children[i]));
    }
    var g = 0;
    while (g < children.length) {
      if (!sigs[g]) { out += walk(children[g], depth); g++; continue; }
      var end = g + 1;
      while (end < children.length && sigs[end] === sigs[g]) end++;
      if (end - g >= 3) {
        for (var k = g; k < end; k++) out += compact(children[k], depth);
      } else {
        for (var k = g; k < end; k++) out += walk(children[k], depth);
      }
      g = end;
    }
    return out;
  }

  function walk(el, depth) {
    if (el.nodeType !== 1) return '';
    if (!isVisible(el)) return '';
    var tag = el.tagName;
    if (['SCRIPT','STYLE','NOSCRIPT','TEMPLATE','SVG'].indexOf(tag) !== -1) return '';

    var out = '';
    var pad = '';
    for (var i = 0; i < depth; i++) pad += '  ';

    var interactive = isInteractive(el);
    if (interactive) {
      ref++;
      el.setAttribute('data-if2ai-ref', String(ref));
      var role = el.getAttribute('role') || tag.toLowerCase();
      var name = el.getAttribute('aria-label') || el.title || el.placeholder
        || directText(el) || el.value || '';
      var label = name.slice(0, 60);

      var flags = [];
      if (el.type && el.type !== 'submit' && tag === 'INPUT') flags.push(el.type);
      if (tag === 'INPUT' && el.value) flags.push('value="' + el.value.slice(0,30) + '"');
      if (el.checked) flags.push('checked');
      if (el.disabled) flags.push('disabled');
      if (el.getAttribute('aria-selected') === 'true') flags.push('selected');
      if (el.getAttribute('aria-expanded')) flags.push('expanded=' + el.getAttribute('aria-expanded'));
      if (tag === 'A' && el.href) flags.push('href="' + el.href.slice(0,80) + '"');

      var extra = flags.length ? ' (' + flags.join(', ') + ')' : '';
      out += pad + '[' + ref + '] ' + role + ' "' + label + '"' + extra + '\n';
    } else if (/^H[1-6]/.test(tag)) {
      var hText = directText(el);
      if (hText) out += pad + tag.toLowerCase() + ': ' + hText + '\n';
    } else if (tag === 'IMG') {
      out += pad + 'img "' + (el.alt || '').slice(0,40) + '"\n';
    } else if (['P','SPAN','DIV','LI','TD','TH','LABEL'].indexOf(tag) !== -1) {
      var txt = directText(el);
      if (txt && txt.length > 2 && !el.querySelector('a,button,input,textarea,select,[role]')) {
        out += pad + 'text: ' + txt + '\n';
      }
    }

    out += walkChildren(el, interactive ? depth + 1 : depth);
    return out;
  }

  var tree = walk(document.body, 0);

  // Phase 7C, slice 7C.7 — descend into same-origin iframes so the LLM
  // sees their interactive elements (currently invisible: contentDocument
  // throws SecurityError on cross-origin frames, in which case we emit
  // a single placeholder line so the LLM at least knows the iframe
  // exists and what its src is).
  function walkIframes(rootDoc, depth) {
    var out = '';
    var iframes = rootDoc.querySelectorAll('iframe');
    var frameIdx = 0;
    iframes.forEach(function(frame) {
      frameIdx += 1;
      var src = frame.src || frame.getAttribute('src') || '<inline-srcdoc>';
      var doc = null;
      try { doc = frame.contentDocument; } catch (e) { doc = null; }
      if (doc && doc.body) {
        out += '\n--- iframe #' + frameIdx + ' (same-origin): ' + src + ' ---\n';
        out += walk(doc.body, depth + 1);
        out += walkIframes(doc, depth + 1);
      } else {
        out += '\n--- iframe #' + frameIdx + ' (cross-origin, not directly accessible): ' + src + ' ---\n';
      }
    });
    return out;
  }
  tree += walkIframes(document, 0);

  // Hard limit: keep 80% head + 20% tail, truncate at line boundaries.
  if (tree.length > MAX_TREE) {
    var h = tree.lastIndexOf('\n', Math.floor(MAX_TREE * 0.8));
    if (h < MAX_TREE * 0.4) h = Math.floor(MAX_TREE * 0.8);
    var tl = tree.indexOf('\n', tree.length - Math.floor(MAX_TREE * 0.2));
    if (tl < 0) tl = tree.length - Math.floor(MAX_TREE * 0.2);
    tree = tree.slice(0, h) + '\n\n[... ' + (tl - h) + ' chars omitted ...]\n\n' + tree.slice(tl);
  }

  return {
    title: document.title,
    currentUrl: location.href,
    text: 'Page: ' + document.title + '\nURL: ' + location.href + '\n\n' + tree
  };
})()"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_script_contains_iframe_walker() {
        // Phase 7C, slice 7C.7 — guard against accidental removal of
        // the same-origin iframe walker.
        assert!(SNAPSHOT_SCRIPT.contains("function walkIframes"));
        assert!(SNAPSHOT_SCRIPT.contains("contentDocument"));
        assert!(SNAPSHOT_SCRIPT.contains("(same-origin)"));
        assert!(SNAPSHOT_SCRIPT.contains("(cross-origin"));
    }

    #[test]
    fn snapshot_script_uses_data_if2ai_ref_attribute() {
        // The ref attribute name is used by every interactive action
        // (click / type / select); a typo here breaks the whole tool.
        assert!(SNAPSHOT_SCRIPT.contains("data-if2ai-ref"));
        assert!(!SNAPSHOT_SCRIPT.contains("data-hana-ref"));
    }

    #[test]
    fn snapshot_script_enforces_max_tree_size() {
        assert!(SNAPSHOT_SCRIPT.contains("MAX_TREE"));
        assert!(SNAPSHOT_SCRIPT.contains("30000"));
    }
}
