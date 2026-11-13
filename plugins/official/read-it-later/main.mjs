var __create = Object.create;
var __getProtoOf = Object.getPrototypeOf;
var __defProp = Object.defineProperty;
var __getOwnPropNames = Object.getOwnPropertyNames;
var __hasOwnProp = Object.prototype.hasOwnProperty;
function __accessProp(key) {
  return this[key];
}
var __toESMCache_node;
var __toESMCache_esm;
var __toESM = (mod, isNodeMode, target) => {
  var canCache = mod != null && typeof mod === "object";
  if (canCache) {
    var cache = isNodeMode ? __toESMCache_node ??= new WeakMap : __toESMCache_esm ??= new WeakMap;
    var cached = cache.get(mod);
    if (cached)
      return cached;
  }
  target = mod != null ? __create(__getProtoOf(mod)) : {};
  const to = isNodeMode || !mod || !mod.__esModule ? __defProp(target, "default", { value: mod, enumerable: true }) : target;
  for (let key of __getOwnPropNames(mod))
    if (!__hasOwnProp.call(to, key))
      __defProp(to, key, {
        get: __accessProp.bind(mod, key),
        enumerable: true
      });
  if (canCache)
    cache.set(mod, to);
  return to;
};
var __commonJS = (cb, mod) => () => (mod || cb((mod = { exports: {} }).exports, mod), mod.exports);

// node_modules/@mozilla/readability/Readability.js
var require_Readability = __commonJS((exports, module) => {
  function Readability(doc, options) {
    if (options && options.documentElement) {
      doc = options;
      options = arguments[2];
    } else if (!doc || !doc.documentElement) {
      throw new Error("First argument to Readability constructor should be a document object.");
    }
    options = options || {};
    this._doc = doc;
    this._docJSDOMParser = this._doc.firstChild.__JSDOMParser__;
    this._articleTitle = null;
    this._articleByline = null;
    this._articleDir = null;
    this._articleSiteName = null;
    this._attempts = [];
    this._metadata = {};
    this._debug = !!options.debug;
    this._maxElemsToParse = options.maxElemsToParse || this.DEFAULT_MAX_ELEMS_TO_PARSE;
    this._nbTopCandidates = options.nbTopCandidates || this.DEFAULT_N_TOP_CANDIDATES;
    this._charThreshold = options.charThreshold || this.DEFAULT_CHAR_THRESHOLD;
    this._classesToPreserve = this.CLASSES_TO_PRESERVE.concat(options.classesToPreserve || []);
    this._keepClasses = !!options.keepClasses;
    this._serializer = options.serializer || function(el) {
      return el.innerHTML;
    };
    this._disableJSONLD = !!options.disableJSONLD;
    this._allowedVideoRegex = options.allowedVideoRegex || this.REGEXPS.videos;
    this._linkDensityModifier = options.linkDensityModifier || 0;
    this._flags = this.FLAG_STRIP_UNLIKELYS | this.FLAG_WEIGHT_CLASSES | this.FLAG_CLEAN_CONDITIONALLY;
    if (this._debug) {
      let logNode = function(node) {
        if (node.nodeType == node.TEXT_NODE) {
          return `${node.nodeName} ("${node.textContent}")`;
        }
        let attrPairs = Array.from(node.attributes || [], function(attr) {
          return `${attr.name}="${attr.value}"`;
        }).join(" ");
        return `<${node.localName} ${attrPairs}>`;
      };
      this.log = function() {
        if (typeof console !== "undefined") {
          let args = Array.from(arguments, (arg) => {
            if (arg && arg.nodeType == this.ELEMENT_NODE) {
              return logNode(arg);
            }
            return arg;
          });
          args.unshift("Reader: (Readability)");
          console.log(...args);
        } else if (typeof dump !== "undefined") {
          var msg = Array.prototype.map.call(arguments, function(x) {
            return x && x.nodeName ? logNode(x) : x;
          }).join(" ");
          dump("Reader: (Readability) " + msg + `
`);
        }
      };
    } else {
      this.log = function() {};
    }
  }
  Readability.prototype = {
    FLAG_STRIP_UNLIKELYS: 1,
    FLAG_WEIGHT_CLASSES: 2,
    FLAG_CLEAN_CONDITIONALLY: 4,
    ELEMENT_NODE: 1,
    TEXT_NODE: 3,
    DEFAULT_MAX_ELEMS_TO_PARSE: 0,
    DEFAULT_N_TOP_CANDIDATES: 5,
    DEFAULT_TAGS_TO_SCORE: "section,h2,h3,h4,h5,h6,p,td,pre".toUpperCase().split(","),
    DEFAULT_CHAR_THRESHOLD: 500,
    REGEXPS: {
      unlikelyCandidates: /-ad-|ai2html|banner|breadcrumbs|combx|comment|community|cover-wrap|disqus|extra|footer|gdpr|header|legends|menu|related|remark|replies|rss|shoutbox|sidebar|skyscraper|social|sponsor|supplemental|ad-break|agegate|pagination|pager|popup|yom-remote/i,
      okMaybeItsACandidate: /and|article|body|column|content|main|shadow/i,
      positive: /article|body|content|entry|hentry|h-entry|main|page|pagination|post|text|blog|story/i,
      negative: /-ad-|hidden|^hid$| hid$| hid |^hid |banner|combx|comment|com-|contact|footer|gdpr|masthead|media|meta|outbrain|promo|related|scroll|share|shoutbox|sidebar|skyscraper|sponsor|shopping|tags|widget/i,
      extraneous: /print|archive|comment|discuss|e[\-]?mail|share|reply|all|login|sign|single|utility/i,
      byline: /byline|author|dateline|writtenby|p-author/i,
      replaceFonts: /<(\/?)font[^>]*>/gi,
      normalize: /\s{2,}/g,
      videos: /\/\/(www\.)?((dailymotion|youtube|youtube-nocookie|player\.vimeo|v\.qq)\.com|(archive|upload\.wikimedia)\.org|player\.twitch\.tv)/i,
      shareElements: /(\b|_)(share|sharedaddy)(\b|_)/i,
      nextLink: /(next|weiter|continue|>([^\|]|$)|»([^\|]|$))/i,
      prevLink: /(prev|earl|old|new|<|«)/i,
      tokenize: /\W+/g,
      whitespace: /^\s*$/,
      hasContent: /\S$/,
      hashUrl: /^#.+/,
      srcsetUrl: /(\S+)(\s+[\d.]+[xw])?(\s*(?:,|$))/g,
      b64DataUrl: /^data:\s*([^\s;,]+)\s*;\s*base64\s*,/i,
      commas: /\u002C|\u060C|\uFE50|\uFE10|\uFE11|\u2E41|\u2E34|\u2E32|\uFF0C/g,
      jsonLdArticleTypes: /^Article|AdvertiserContentArticle|NewsArticle|AnalysisNewsArticle|AskPublicNewsArticle|BackgroundNewsArticle|OpinionNewsArticle|ReportageNewsArticle|ReviewNewsArticle|Report|SatiricalArticle|ScholarlyArticle|MedicalScholarlyArticle|SocialMediaPosting|BlogPosting|LiveBlogPosting|DiscussionForumPosting|TechArticle|APIReference$/,
      adWords: /^(ad(vertising|vertisement)?|pub(licité)?|werb(ung)?|广告|Реклама|Anuncio)$/iu,
      loadingWords: /^((loading|正在加载|Загрузка|chargement|cargando)(…|\.\.\.)?)$/iu
    },
    UNLIKELY_ROLES: [
      "menu",
      "menubar",
      "complementary",
      "navigation",
      "alert",
      "alertdialog",
      "dialog"
    ],
    DIV_TO_P_ELEMS: new Set([
      "BLOCKQUOTE",
      "DL",
      "DIV",
      "IMG",
      "OL",
      "P",
      "PRE",
      "TABLE",
      "UL"
    ]),
    ALTER_TO_DIV_EXCEPTIONS: ["DIV", "ARTICLE", "SECTION", "P", "OL", "UL"],
    PRESENTATIONAL_ATTRIBUTES: [
      "align",
      "background",
      "bgcolor",
      "border",
      "cellpadding",
      "cellspacing",
      "frame",
      "hspace",
      "rules",
      "style",
      "valign",
      "vspace"
    ],
    DEPRECATED_SIZE_ATTRIBUTE_ELEMS: ["TABLE", "TH", "TD", "HR", "PRE"],
    PHRASING_ELEMS: [
      "ABBR",
      "AUDIO",
      "B",
      "BDO",
      "BR",
      "BUTTON",
      "CITE",
      "CODE",
      "DATA",
      "DATALIST",
      "DFN",
      "EM",
      "EMBED",
      "I",
      "IMG",
      "INPUT",
      "KBD",
      "LABEL",
      "MARK",
      "MATH",
      "METER",
      "NOSCRIPT",
      "OBJECT",
      "OUTPUT",
      "PROGRESS",
      "Q",
      "RUBY",
      "SAMP",
      "SCRIPT",
      "SELECT",
      "SMALL",
      "SPAN",
      "STRONG",
      "SUB",
      "SUP",
      "TEXTAREA",
      "TIME",
      "VAR",
      "WBR"
    ],
    CLASSES_TO_PRESERVE: ["page"],
    HTML_ESCAPE_MAP: {
      lt: "<",
      gt: ">",
      amp: "&",
      quot: '"',
      apos: "'"
    },
    _postProcessContent(articleContent) {
      this._fixRelativeUris(articleContent);
      this._simplifyNestedElements(articleContent);
      if (!this._keepClasses) {
        this._cleanClasses(articleContent);
      }
    },
    _removeNodes(nodeList, filterFn) {
      if (this._docJSDOMParser && nodeList._isLiveNodeList) {
        throw new Error("Do not pass live node lists to _removeNodes");
      }
      for (var i = nodeList.length - 1;i >= 0; i--) {
        var node = nodeList[i];
        var parentNode = node.parentNode;
        if (parentNode) {
          if (!filterFn || filterFn.call(this, node, i, nodeList)) {
            parentNode.removeChild(node);
          }
        }
      }
    },
    _replaceNodeTags(nodeList, newTagName) {
      if (this._docJSDOMParser && nodeList._isLiveNodeList) {
        throw new Error("Do not pass live node lists to _replaceNodeTags");
      }
      for (const node of nodeList) {
        this._setNodeTag(node, newTagName);
      }
    },
    _forEachNode(nodeList, fn) {
      Array.prototype.forEach.call(nodeList, fn, this);
    },
    _findNode(nodeList, fn) {
      return Array.prototype.find.call(nodeList, fn, this);
    },
    _someNode(nodeList, fn) {
      return Array.prototype.some.call(nodeList, fn, this);
    },
    _everyNode(nodeList, fn) {
      return Array.prototype.every.call(nodeList, fn, this);
    },
    _getAllNodesWithTag(node, tagNames) {
      if (node.querySelectorAll) {
        return node.querySelectorAll(tagNames.join(","));
      }
      return [].concat.apply([], tagNames.map(function(tag) {
        var collection = node.getElementsByTagName(tag);
        return Array.isArray(collection) ? collection : Array.from(collection);
      }));
    },
    _cleanClasses(node) {
      var classesToPreserve = this._classesToPreserve;
      var className = (node.getAttribute("class") || "").split(/\s+/).filter((cls) => classesToPreserve.includes(cls)).join(" ");
      if (className) {
        node.setAttribute("class", className);
      } else {
        node.removeAttribute("class");
      }
      for (node = node.firstElementChild;node; node = node.nextElementSibling) {
        this._cleanClasses(node);
      }
    },
    _isUrl(str) {
      try {
        new URL(str);
        return true;
      } catch {
        return false;
      }
    },
    _fixRelativeUris(articleContent) {
      var baseURI = this._doc.baseURI;
      var documentURI = this._doc.documentURI;
      function toAbsoluteURI(uri) {
        if (baseURI == documentURI && uri.charAt(0) == "#") {
          return uri;
        }
        try {
          return new URL(uri, baseURI).href;
        } catch (ex) {}
        return uri;
      }
      var links = this._getAllNodesWithTag(articleContent, ["a"]);
      this._forEachNode(links, function(link) {
        var href = link.getAttribute("href");
        if (href) {
          if (href.indexOf("javascript:") === 0) {
            if (link.childNodes.length === 1 && link.childNodes[0].nodeType === this.TEXT_NODE) {
              var text = this._doc.createTextNode(link.textContent);
              link.parentNode.replaceChild(text, link);
            } else {
              var container = this._doc.createElement("span");
              while (link.firstChild) {
                container.appendChild(link.firstChild);
              }
              link.parentNode.replaceChild(container, link);
            }
          } else {
            link.setAttribute("href", toAbsoluteURI(href));
          }
        }
      });
      var medias = this._getAllNodesWithTag(articleContent, [
        "img",
        "picture",
        "figure",
        "video",
        "audio",
        "source"
      ]);
      this._forEachNode(medias, function(media) {
        var src = media.getAttribute("src");
        var poster = media.getAttribute("poster");
        var srcset = media.getAttribute("srcset");
        if (src) {
          media.setAttribute("src", toAbsoluteURI(src));
        }
        if (poster) {
          media.setAttribute("poster", toAbsoluteURI(poster));
        }
        if (srcset) {
          var newSrcset = srcset.replace(this.REGEXPS.srcsetUrl, function(_, p1, p2, p3) {
            return toAbsoluteURI(p1) + (p2 || "") + p3;
          });
          media.setAttribute("srcset", newSrcset);
        }
      });
    },
    _simplifyNestedElements(articleContent) {
      var node = articleContent;
      while (node) {
        if (node.parentNode && ["DIV", "SECTION"].includes(node.tagName) && !(node.id && node.id.startsWith("readability"))) {
          if (this._isElementWithoutContent(node)) {
            node = this._removeAndGetNext(node);
            continue;
          } else if (this._hasSingleTagInsideElement(node, "DIV") || this._hasSingleTagInsideElement(node, "SECTION")) {
            var child = node.children[0];
            for (var i = 0;i < node.attributes.length; i++) {
              child.setAttributeNode(node.attributes[i].cloneNode());
            }
            node.parentNode.replaceChild(child, node);
            node = child;
            continue;
          }
        }
        node = this._getNextNode(node);
      }
    },
    _getArticleTitle() {
      var doc = this._doc;
      var curTitle = "";
      var origTitle = "";
      try {
        curTitle = origTitle = doc.title.trim();
        if (typeof curTitle !== "string") {
          curTitle = origTitle = this._getInnerText(doc.getElementsByTagName("title")[0]);
        }
      } catch (e) {}
      var titleHadHierarchicalSeparators = false;
      function wordCount(str) {
        return str.split(/\s+/).length;
      }
      if (/ [\|\-\\\/>»] /.test(curTitle)) {
        titleHadHierarchicalSeparators = / [\\\/>»] /.test(curTitle);
        let allSeparators = Array.from(origTitle.matchAll(/ [\|\-\\\/>»] /gi));
        curTitle = origTitle.substring(0, allSeparators.pop().index);
        if (wordCount(curTitle) < 3) {
          curTitle = origTitle.replace(/^[^\|\-\\\/>»]*[\|\-\\\/>»]/gi, "");
        }
      } else if (curTitle.includes(": ")) {
        var headings = this._getAllNodesWithTag(doc, ["h1", "h2"]);
        var trimmedTitle = curTitle.trim();
        var match = this._someNode(headings, function(heading) {
          return heading.textContent.trim() === trimmedTitle;
        });
        if (!match) {
          curTitle = origTitle.substring(origTitle.lastIndexOf(":") + 1);
          if (wordCount(curTitle) < 3) {
            curTitle = origTitle.substring(origTitle.indexOf(":") + 1);
          } else if (wordCount(origTitle.substr(0, origTitle.indexOf(":"))) > 5) {
            curTitle = origTitle;
          }
        }
      } else if (curTitle.length > 150 || curTitle.length < 15) {
        var hOnes = doc.getElementsByTagName("h1");
        if (hOnes.length === 1) {
          curTitle = this._getInnerText(hOnes[0]);
        }
      }
      curTitle = curTitle.trim().replace(this.REGEXPS.normalize, " ");
      var curTitleWordCount = wordCount(curTitle);
      if (curTitleWordCount <= 4 && (!titleHadHierarchicalSeparators || curTitleWordCount != wordCount(origTitle.replace(/[\|\-\\\/>»]+/g, "")) - 1)) {
        curTitle = origTitle;
      }
      return curTitle;
    },
    _prepDocument() {
      var doc = this._doc;
      this._removeNodes(this._getAllNodesWithTag(doc, ["style"]));
      if (doc.body) {
        this._replaceBrs(doc.body);
      }
      this._replaceNodeTags(this._getAllNodesWithTag(doc, ["font"]), "SPAN");
    },
    _nextNode(node) {
      var next = node;
      while (next && next.nodeType != this.ELEMENT_NODE && this.REGEXPS.whitespace.test(next.textContent)) {
        next = next.nextSibling;
      }
      return next;
    },
    _replaceBrs(elem) {
      this._forEachNode(this._getAllNodesWithTag(elem, ["br"]), function(br) {
        var next = br.nextSibling;
        var replaced = false;
        while ((next = this._nextNode(next)) && next.tagName == "BR") {
          replaced = true;
          var brSibling = next.nextSibling;
          next.remove();
          next = brSibling;
        }
        if (replaced) {
          var p = this._doc.createElement("p");
          br.parentNode.replaceChild(p, br);
          next = p.nextSibling;
          while (next) {
            if (next.tagName == "BR") {
              var nextElem = this._nextNode(next.nextSibling);
              if (nextElem && nextElem.tagName == "BR") {
                break;
              }
            }
            if (!this._isPhrasingContent(next)) {
              break;
            }
            var sibling = next.nextSibling;
            p.appendChild(next);
            next = sibling;
          }
          while (p.lastChild && this._isWhitespace(p.lastChild)) {
            p.lastChild.remove();
          }
          if (p.parentNode.tagName === "P") {
            this._setNodeTag(p.parentNode, "DIV");
          }
        }
      });
    },
    _setNodeTag(node, tag) {
      this.log("_setNodeTag", node, tag);
      if (this._docJSDOMParser) {
        node.localName = tag.toLowerCase();
        node.tagName = tag.toUpperCase();
        return node;
      }
      var replacement = node.ownerDocument.createElement(tag);
      while (node.firstChild) {
        replacement.appendChild(node.firstChild);
      }
      node.parentNode.replaceChild(replacement, node);
      if (node.readability) {
        replacement.readability = node.readability;
      }
      for (var i = 0;i < node.attributes.length; i++) {
        replacement.setAttributeNode(node.attributes[i].cloneNode());
      }
      return replacement;
    },
    _prepArticle(articleContent) {
      this._cleanStyles(articleContent);
      this._markDataTables(articleContent);
      this._fixLazyImages(articleContent);
      this._cleanConditionally(articleContent, "form");
      this._cleanConditionally(articleContent, "fieldset");
      this._clean(articleContent, "object");
      this._clean(articleContent, "embed");
      this._clean(articleContent, "footer");
      this._clean(articleContent, "link");
      this._clean(articleContent, "aside");
      var shareElementThreshold = this.DEFAULT_CHAR_THRESHOLD;
      this._forEachNode(articleContent.children, function(topCandidate) {
        this._cleanMatchedNodes(topCandidate, function(node, matchString) {
          return this.REGEXPS.shareElements.test(matchString) && node.textContent.length < shareElementThreshold;
        });
      });
      this._clean(articleContent, "iframe");
      this._clean(articleContent, "input");
      this._clean(articleContent, "textarea");
      this._clean(articleContent, "select");
      this._clean(articleContent, "button");
      this._cleanHeaders(articleContent);
      this._cleanConditionally(articleContent, "table");
      this._cleanConditionally(articleContent, "ul");
      this._cleanConditionally(articleContent, "div");
      this._replaceNodeTags(this._getAllNodesWithTag(articleContent, ["h1"]), "h2");
      this._removeNodes(this._getAllNodesWithTag(articleContent, ["p"]), function(paragraph) {
        var contentElementCount = this._getAllNodesWithTag(paragraph, [
          "img",
          "embed",
          "object",
          "iframe"
        ]).length;
        return contentElementCount === 0 && !this._getInnerText(paragraph, false);
      });
      this._forEachNode(this._getAllNodesWithTag(articleContent, ["br"]), function(br) {
        var next = this._nextNode(br.nextSibling);
        if (next && next.tagName == "P") {
          br.remove();
        }
      });
      this._forEachNode(this._getAllNodesWithTag(articleContent, ["table"]), function(table) {
        var tbody = this._hasSingleTagInsideElement(table, "TBODY") ? table.firstElementChild : table;
        if (this._hasSingleTagInsideElement(tbody, "TR")) {
          var row = tbody.firstElementChild;
          if (this._hasSingleTagInsideElement(row, "TD")) {
            var cell = row.firstElementChild;
            cell = this._setNodeTag(cell, this._everyNode(cell.childNodes, this._isPhrasingContent) ? "P" : "DIV");
            table.parentNode.replaceChild(cell, table);
          }
        }
      });
    },
    _initializeNode(node) {
      node.readability = { contentScore: 0 };
      switch (node.tagName) {
        case "DIV":
          node.readability.contentScore += 5;
          break;
        case "PRE":
        case "TD":
        case "BLOCKQUOTE":
          node.readability.contentScore += 3;
          break;
        case "ADDRESS":
        case "OL":
        case "UL":
        case "DL":
        case "DD":
        case "DT":
        case "LI":
        case "FORM":
          node.readability.contentScore -= 3;
          break;
        case "H1":
        case "H2":
        case "H3":
        case "H4":
        case "H5":
        case "H6":
        case "TH":
          node.readability.contentScore -= 5;
          break;
      }
      node.readability.contentScore += this._getClassWeight(node);
    },
    _removeAndGetNext(node) {
      var nextNode = this._getNextNode(node, true);
      node.remove();
      return nextNode;
    },
    _getNextNode(node, ignoreSelfAndKids) {
      if (!ignoreSelfAndKids && node.firstElementChild) {
        return node.firstElementChild;
      }
      if (node.nextElementSibling) {
        return node.nextElementSibling;
      }
      do {
        node = node.parentNode;
      } while (node && !node.nextElementSibling);
      return node && node.nextElementSibling;
    },
    _textSimilarity(textA, textB) {
      var tokensA = textA.toLowerCase().split(this.REGEXPS.tokenize).filter(Boolean);
      var tokensB = textB.toLowerCase().split(this.REGEXPS.tokenize).filter(Boolean);
      if (!tokensA.length || !tokensB.length) {
        return 0;
      }
      var uniqTokensB = tokensB.filter((token) => !tokensA.includes(token));
      var distanceB = uniqTokensB.join(" ").length / tokensB.join(" ").length;
      return 1 - distanceB;
    },
    _isValidByline(node, matchString) {
      var rel = node.getAttribute("rel");
      var itemprop = node.getAttribute("itemprop");
      var bylineLength = node.textContent.trim().length;
      return (rel === "author" || itemprop && itemprop.includes("author") || this.REGEXPS.byline.test(matchString)) && !!bylineLength && bylineLength < 100;
    },
    _getNodeAncestors(node, maxDepth) {
      maxDepth = maxDepth || 0;
      var i = 0, ancestors = [];
      while (node.parentNode) {
        ancestors.push(node.parentNode);
        if (maxDepth && ++i === maxDepth) {
          break;
        }
        node = node.parentNode;
      }
      return ancestors;
    },
    _grabArticle(page) {
      this.log("**** grabArticle ****");
      var doc = this._doc;
      var isPaging = page !== null;
      page = page ? page : this._doc.body;
      if (!page) {
        this.log("No body found in document. Abort.");
        return null;
      }
      var pageCacheHtml = page.innerHTML;
      while (true) {
        this.log("Starting grabArticle loop");
        var stripUnlikelyCandidates = this._flagIsActive(this.FLAG_STRIP_UNLIKELYS);
        var elementsToScore = [];
        var node = this._doc.documentElement;
        let shouldRemoveTitleHeader = true;
        while (node) {
          if (node.tagName === "HTML") {
            this._articleLang = node.getAttribute("lang");
          }
          var matchString = node.className + " " + node.id;
          if (!this._isProbablyVisible(node)) {
            this.log("Removing hidden node - " + matchString);
            node = this._removeAndGetNext(node);
            continue;
          }
          if (node.getAttribute("aria-modal") == "true" && node.getAttribute("role") == "dialog") {
            node = this._removeAndGetNext(node);
            continue;
          }
          if (!this._articleByline && !this._metadata.byline && this._isValidByline(node, matchString)) {
            var endOfSearchMarkerNode = this._getNextNode(node, true);
            var next = this._getNextNode(node);
            var itemPropNameNode = null;
            while (next && next != endOfSearchMarkerNode) {
              var itemprop = next.getAttribute("itemprop");
              if (itemprop && itemprop.includes("name")) {
                itemPropNameNode = next;
                break;
              } else {
                next = this._getNextNode(next);
              }
            }
            this._articleByline = (itemPropNameNode ?? node).textContent.trim();
            node = this._removeAndGetNext(node);
            continue;
          }
          if (shouldRemoveTitleHeader && this._headerDuplicatesTitle(node)) {
            this.log("Removing header: ", node.textContent.trim(), this._articleTitle.trim());
            shouldRemoveTitleHeader = false;
            node = this._removeAndGetNext(node);
            continue;
          }
          if (stripUnlikelyCandidates) {
            if (this.REGEXPS.unlikelyCandidates.test(matchString) && !this.REGEXPS.okMaybeItsACandidate.test(matchString) && !this._hasAncestorTag(node, "table") && !this._hasAncestorTag(node, "code") && node.tagName !== "BODY" && node.tagName !== "A") {
              this.log("Removing unlikely candidate - " + matchString);
              node = this._removeAndGetNext(node);
              continue;
            }
            if (this.UNLIKELY_ROLES.includes(node.getAttribute("role"))) {
              this.log("Removing content with role " + node.getAttribute("role") + " - " + matchString);
              node = this._removeAndGetNext(node);
              continue;
            }
          }
          if ((node.tagName === "DIV" || node.tagName === "SECTION" || node.tagName === "HEADER" || node.tagName === "H1" || node.tagName === "H2" || node.tagName === "H3" || node.tagName === "H4" || node.tagName === "H5" || node.tagName === "H6") && this._isElementWithoutContent(node)) {
            node = this._removeAndGetNext(node);
            continue;
          }
          if (this.DEFAULT_TAGS_TO_SCORE.includes(node.tagName)) {
            elementsToScore.push(node);
          }
          if (node.tagName === "DIV") {
            var p = null;
            var childNode = node.firstChild;
            while (childNode) {
              var nextSibling = childNode.nextSibling;
              if (this._isPhrasingContent(childNode)) {
                if (p !== null) {
                  p.appendChild(childNode);
                } else if (!this._isWhitespace(childNode)) {
                  p = doc.createElement("p");
                  node.replaceChild(p, childNode);
                  p.appendChild(childNode);
                }
              } else if (p !== null) {
                while (p.lastChild && this._isWhitespace(p.lastChild)) {
                  p.lastChild.remove();
                }
                p = null;
              }
              childNode = nextSibling;
            }
            if (this._hasSingleTagInsideElement(node, "P") && this._getLinkDensity(node) < 0.25) {
              var newNode = node.children[0];
              node.parentNode.replaceChild(newNode, node);
              node = newNode;
              elementsToScore.push(node);
            } else if (!this._hasChildBlockElement(node)) {
              node = this._setNodeTag(node, "P");
              elementsToScore.push(node);
            }
          }
          node = this._getNextNode(node);
        }
        var candidates = [];
        this._forEachNode(elementsToScore, function(elementToScore) {
          if (!elementToScore.parentNode || typeof elementToScore.parentNode.tagName === "undefined") {
            return;
          }
          var innerText = this._getInnerText(elementToScore);
          if (innerText.length < 25) {
            return;
          }
          var ancestors2 = this._getNodeAncestors(elementToScore, 5);
          if (ancestors2.length === 0) {
            return;
          }
          var contentScore = 0;
          contentScore += 1;
          contentScore += innerText.split(this.REGEXPS.commas).length;
          contentScore += Math.min(Math.floor(innerText.length / 100), 3);
          this._forEachNode(ancestors2, function(ancestor, level) {
            if (!ancestor.tagName || !ancestor.parentNode || typeof ancestor.parentNode.tagName === "undefined") {
              return;
            }
            if (typeof ancestor.readability === "undefined") {
              this._initializeNode(ancestor);
              candidates.push(ancestor);
            }
            if (level === 0) {
              var scoreDivider = 1;
            } else if (level === 1) {
              scoreDivider = 2;
            } else {
              scoreDivider = level * 3;
            }
            ancestor.readability.contentScore += contentScore / scoreDivider;
          });
        });
        var topCandidates = [];
        for (var c = 0, cl = candidates.length;c < cl; c += 1) {
          var candidate = candidates[c];
          var candidateScore = candidate.readability.contentScore * (1 - this._getLinkDensity(candidate));
          candidate.readability.contentScore = candidateScore;
          this.log("Candidate:", candidate, "with score " + candidateScore);
          for (var t = 0;t < this._nbTopCandidates; t++) {
            var aTopCandidate = topCandidates[t];
            if (!aTopCandidate || candidateScore > aTopCandidate.readability.contentScore) {
              topCandidates.splice(t, 0, candidate);
              if (topCandidates.length > this._nbTopCandidates) {
                topCandidates.pop();
              }
              break;
            }
          }
        }
        var topCandidate = topCandidates[0] || null;
        var neededToCreateTopCandidate = false;
        var parentOfTopCandidate;
        if (topCandidate === null || topCandidate.tagName === "BODY") {
          topCandidate = doc.createElement("DIV");
          neededToCreateTopCandidate = true;
          while (page.firstChild) {
            this.log("Moving child out:", page.firstChild);
            topCandidate.appendChild(page.firstChild);
          }
          page.appendChild(topCandidate);
          this._initializeNode(topCandidate);
        } else if (topCandidate) {
          var alternativeCandidateAncestors = [];
          for (var i = 1;i < topCandidates.length; i++) {
            if (topCandidates[i].readability.contentScore / topCandidate.readability.contentScore >= 0.75) {
              alternativeCandidateAncestors.push(this._getNodeAncestors(topCandidates[i]));
            }
          }
          var MINIMUM_TOPCANDIDATES = 3;
          if (alternativeCandidateAncestors.length >= MINIMUM_TOPCANDIDATES) {
            parentOfTopCandidate = topCandidate.parentNode;
            while (parentOfTopCandidate.tagName !== "BODY") {
              var listsContainingThisAncestor = 0;
              for (var ancestorIndex = 0;ancestorIndex < alternativeCandidateAncestors.length && listsContainingThisAncestor < MINIMUM_TOPCANDIDATES; ancestorIndex++) {
                listsContainingThisAncestor += Number(alternativeCandidateAncestors[ancestorIndex].includes(parentOfTopCandidate));
              }
              if (listsContainingThisAncestor >= MINIMUM_TOPCANDIDATES) {
                topCandidate = parentOfTopCandidate;
                break;
              }
              parentOfTopCandidate = parentOfTopCandidate.parentNode;
            }
          }
          if (!topCandidate.readability) {
            this._initializeNode(topCandidate);
          }
          parentOfTopCandidate = topCandidate.parentNode;
          var lastScore = topCandidate.readability.contentScore;
          var scoreThreshold = lastScore / 3;
          while (parentOfTopCandidate.tagName !== "BODY") {
            if (!parentOfTopCandidate.readability) {
              parentOfTopCandidate = parentOfTopCandidate.parentNode;
              continue;
            }
            var parentScore = parentOfTopCandidate.readability.contentScore;
            if (parentScore < scoreThreshold) {
              break;
            }
            if (parentScore > lastScore) {
              topCandidate = parentOfTopCandidate;
              break;
            }
            lastScore = parentOfTopCandidate.readability.contentScore;
            parentOfTopCandidate = parentOfTopCandidate.parentNode;
          }
          parentOfTopCandidate = topCandidate.parentNode;
          while (parentOfTopCandidate.tagName != "BODY" && parentOfTopCandidate.children.length == 1) {
            topCandidate = parentOfTopCandidate;
            parentOfTopCandidate = topCandidate.parentNode;
          }
          if (!topCandidate.readability) {
            this._initializeNode(topCandidate);
          }
        }
        var articleContent = doc.createElement("DIV");
        if (isPaging) {
          articleContent.id = "readability-content";
        }
        var siblingScoreThreshold = Math.max(10, topCandidate.readability.contentScore * 0.2);
        parentOfTopCandidate = topCandidate.parentNode;
        var siblings = parentOfTopCandidate.children;
        for (var s = 0, sl = siblings.length;s < sl; s++) {
          var sibling = siblings[s];
          var append = false;
          this.log("Looking at sibling node:", sibling, sibling.readability ? "with score " + sibling.readability.contentScore : "");
          this.log("Sibling has score", sibling.readability ? sibling.readability.contentScore : "Unknown");
          if (sibling === topCandidate) {
            append = true;
          } else {
            var contentBonus = 0;
            if (sibling.className === topCandidate.className && topCandidate.className !== "") {
              contentBonus += topCandidate.readability.contentScore * 0.2;
            }
            if (sibling.readability && sibling.readability.contentScore + contentBonus >= siblingScoreThreshold) {
              append = true;
            } else if (sibling.nodeName === "P") {
              var linkDensity = this._getLinkDensity(sibling);
              var nodeContent = this._getInnerText(sibling);
              var nodeLength = nodeContent.length;
              if (nodeLength > 80 && linkDensity < 0.25) {
                append = true;
              } else if (nodeLength < 80 && nodeLength > 0 && linkDensity === 0 && nodeContent.search(/\.( |$)/) !== -1) {
                append = true;
              }
            }
          }
          if (append) {
            this.log("Appending node:", sibling);
            if (!this.ALTER_TO_DIV_EXCEPTIONS.includes(sibling.nodeName)) {
              this.log("Altering sibling:", sibling, "to div.");
              sibling = this._setNodeTag(sibling, "DIV");
            }
            articleContent.appendChild(sibling);
            siblings = parentOfTopCandidate.children;
            s -= 1;
            sl -= 1;
          }
        }
        if (this._debug) {
          this.log("Article content pre-prep: " + articleContent.innerHTML);
        }
        this._prepArticle(articleContent);
        if (this._debug) {
          this.log("Article content post-prep: " + articleContent.innerHTML);
        }
        if (neededToCreateTopCandidate) {
          topCandidate.id = "readability-page-1";
          topCandidate.className = "page";
        } else {
          var div = doc.createElement("DIV");
          div.id = "readability-page-1";
          div.className = "page";
          while (articleContent.firstChild) {
            div.appendChild(articleContent.firstChild);
          }
          articleContent.appendChild(div);
        }
        if (this._debug) {
          this.log("Article content after paging: " + articleContent.innerHTML);
        }
        var parseSuccessful = true;
        var textLength = this._getInnerText(articleContent, true).length;
        if (textLength < this._charThreshold) {
          parseSuccessful = false;
          page.innerHTML = pageCacheHtml;
          this._attempts.push({
            articleContent,
            textLength
          });
          if (this._flagIsActive(this.FLAG_STRIP_UNLIKELYS)) {
            this._removeFlag(this.FLAG_STRIP_UNLIKELYS);
          } else if (this._flagIsActive(this.FLAG_WEIGHT_CLASSES)) {
            this._removeFlag(this.FLAG_WEIGHT_CLASSES);
          } else if (this._flagIsActive(this.FLAG_CLEAN_CONDITIONALLY)) {
            this._removeFlag(this.FLAG_CLEAN_CONDITIONALLY);
          } else {
            this._attempts.sort(function(a, b) {
              return b.textLength - a.textLength;
            });
            if (!this._attempts[0].textLength) {
              return null;
            }
            articleContent = this._attempts[0].articleContent;
            parseSuccessful = true;
          }
        }
        if (parseSuccessful) {
          var ancestors = [parentOfTopCandidate, topCandidate].concat(this._getNodeAncestors(parentOfTopCandidate));
          this._someNode(ancestors, function(ancestor) {
            if (!ancestor.tagName) {
              return false;
            }
            var articleDir = ancestor.getAttribute("dir");
            if (articleDir) {
              this._articleDir = articleDir;
              return true;
            }
            return false;
          });
          return articleContent;
        }
      }
    },
    _unescapeHtmlEntities(str) {
      if (!str) {
        return str;
      }
      var htmlEscapeMap = this.HTML_ESCAPE_MAP;
      return str.replace(/&(quot|amp|apos|lt|gt);/g, function(_, tag) {
        return htmlEscapeMap[tag];
      }).replace(/&#(?:x([0-9a-f]+)|([0-9]+));/gi, function(_, hex, numStr) {
        var num = parseInt(hex || numStr, hex ? 16 : 10);
        if (num == 0 || num > 1114111 || num >= 55296 && num <= 57343) {
          num = 65533;
        }
        return String.fromCodePoint(num);
      });
    },
    _getJSONLD(doc) {
      var scripts = this._getAllNodesWithTag(doc, ["script"]);
      var metadata;
      this._forEachNode(scripts, function(jsonLdElement) {
        if (!metadata && jsonLdElement.getAttribute("type") === "application/ld+json") {
          try {
            var content = jsonLdElement.textContent.replace(/^\s*<!\[CDATA\[|\]\]>\s*$/g, "");
            var parsed = JSON.parse(content);
            if (Array.isArray(parsed)) {
              parsed = parsed.find((it) => {
                return it["@type"] && it["@type"].match(this.REGEXPS.jsonLdArticleTypes);
              });
              if (!parsed) {
                return;
              }
            }
            var schemaDotOrgRegex = /^https?\:\/\/schema\.org\/?$/;
            var matches = typeof parsed["@context"] === "string" && parsed["@context"].match(schemaDotOrgRegex) || typeof parsed["@context"] === "object" && typeof parsed["@context"]["@vocab"] == "string" && parsed["@context"]["@vocab"].match(schemaDotOrgRegex);
            if (!matches) {
              return;
            }
            if (!parsed["@type"] && Array.isArray(parsed["@graph"])) {
              parsed = parsed["@graph"].find((it) => {
                return (it["@type"] || "").match(this.REGEXPS.jsonLdArticleTypes);
              });
            }
            if (!parsed || !parsed["@type"] || !parsed["@type"].match(this.REGEXPS.jsonLdArticleTypes)) {
              return;
            }
            metadata = {};
            if (typeof parsed.name === "string" && typeof parsed.headline === "string" && parsed.name !== parsed.headline) {
              var title = this._getArticleTitle();
              var nameMatches = this._textSimilarity(parsed.name, title) > 0.75;
              var headlineMatches = this._textSimilarity(parsed.headline, title) > 0.75;
              if (headlineMatches && !nameMatches) {
                metadata.title = parsed.headline;
              } else {
                metadata.title = parsed.name;
              }
            } else if (typeof parsed.name === "string") {
              metadata.title = parsed.name.trim();
            } else if (typeof parsed.headline === "string") {
              metadata.title = parsed.headline.trim();
            }
            if (parsed.author) {
              if (typeof parsed.author.name === "string") {
                metadata.byline = parsed.author.name.trim();
              } else if (Array.isArray(parsed.author) && parsed.author[0] && typeof parsed.author[0].name === "string") {
                metadata.byline = parsed.author.filter(function(author) {
                  return author && typeof author.name === "string";
                }).map(function(author) {
                  return author.name.trim();
                }).join(", ");
              }
            }
            if (typeof parsed.description === "string") {
              metadata.excerpt = parsed.description.trim();
            }
            if (parsed.publisher && typeof parsed.publisher.name === "string") {
              metadata.siteName = parsed.publisher.name.trim();
            }
            if (typeof parsed.datePublished === "string") {
              metadata.datePublished = parsed.datePublished.trim();
            }
          } catch (err) {
            this.log(err.message);
          }
        }
      });
      return metadata ? metadata : {};
    },
    _getArticleMetadata(jsonld) {
      var metadata = {};
      var values = {};
      var metaElements = this._doc.getElementsByTagName("meta");
      var propertyPattern = /\s*(article|dc|dcterm|og|twitter)\s*:\s*(author|creator|description|published_time|title|site_name)\s*/gi;
      var namePattern = /^\s*(?:(dc|dcterm|og|twitter|parsely|weibo:(article|webpage))\s*[-\.:]\s*)?(author|creator|pub-date|description|title|site_name)\s*$/i;
      this._forEachNode(metaElements, function(element) {
        var elementName = element.getAttribute("name");
        var elementProperty = element.getAttribute("property");
        var content = element.getAttribute("content");
        if (!content) {
          return;
        }
        var matches = null;
        var name = null;
        if (elementProperty) {
          matches = elementProperty.match(propertyPattern);
          if (matches) {
            name = matches[0].toLowerCase().replace(/\s/g, "");
            values[name] = content.trim();
          }
        }
        if (!matches && elementName && namePattern.test(elementName)) {
          name = elementName;
          if (content) {
            name = name.toLowerCase().replace(/\s/g, "").replace(/\./g, ":");
            values[name] = content.trim();
          }
        }
      });
      metadata.title = jsonld.title || values["dc:title"] || values["dcterm:title"] || values["og:title"] || values["weibo:article:title"] || values["weibo:webpage:title"] || values.title || values["twitter:title"] || values["parsely-title"];
      if (!metadata.title) {
        metadata.title = this._getArticleTitle();
      }
      const articleAuthor = typeof values["article:author"] === "string" && !this._isUrl(values["article:author"]) ? values["article:author"] : undefined;
      metadata.byline = jsonld.byline || values["dc:creator"] || values["dcterm:creator"] || values.author || values["parsely-author"] || articleAuthor;
      metadata.excerpt = jsonld.excerpt || values["dc:description"] || values["dcterm:description"] || values["og:description"] || values["weibo:article:description"] || values["weibo:webpage:description"] || values.description || values["twitter:description"];
      metadata.siteName = jsonld.siteName || values["og:site_name"];
      metadata.publishedTime = jsonld.datePublished || values["article:published_time"] || values["parsely-pub-date"] || null;
      metadata.title = this._unescapeHtmlEntities(metadata.title);
      metadata.byline = this._unescapeHtmlEntities(metadata.byline);
      metadata.excerpt = this._unescapeHtmlEntities(metadata.excerpt);
      metadata.siteName = this._unescapeHtmlEntities(metadata.siteName);
      metadata.publishedTime = this._unescapeHtmlEntities(metadata.publishedTime);
      return metadata;
    },
    _isSingleImage(node) {
      while (node) {
        if (node.tagName === "IMG") {
          return true;
        }
        if (node.children.length !== 1 || node.textContent.trim() !== "") {
          return false;
        }
        node = node.children[0];
      }
      return false;
    },
    _unwrapNoscriptImages(doc) {
      var imgs = Array.from(doc.getElementsByTagName("img"));
      this._forEachNode(imgs, function(img) {
        for (var i = 0;i < img.attributes.length; i++) {
          var attr = img.attributes[i];
          switch (attr.name) {
            case "src":
            case "srcset":
            case "data-src":
            case "data-srcset":
              return;
          }
          if (/\.(jpg|jpeg|png|webp)/i.test(attr.value)) {
            return;
          }
        }
        img.remove();
      });
      var noscripts = Array.from(doc.getElementsByTagName("noscript"));
      this._forEachNode(noscripts, function(noscript) {
        if (!this._isSingleImage(noscript)) {
          return;
        }
        var tmp = doc.createElement("div");
        tmp.innerHTML = noscript.innerHTML;
        var prevElement = noscript.previousElementSibling;
        if (prevElement && this._isSingleImage(prevElement)) {
          var prevImg = prevElement;
          if (prevImg.tagName !== "IMG") {
            prevImg = prevElement.getElementsByTagName("img")[0];
          }
          var newImg = tmp.getElementsByTagName("img")[0];
          for (var i = 0;i < prevImg.attributes.length; i++) {
            var attr = prevImg.attributes[i];
            if (attr.value === "") {
              continue;
            }
            if (attr.name === "src" || attr.name === "srcset" || /\.(jpg|jpeg|png|webp)/i.test(attr.value)) {
              if (newImg.getAttribute(attr.name) === attr.value) {
                continue;
              }
              var attrName = attr.name;
              if (newImg.hasAttribute(attrName)) {
                attrName = "data-old-" + attrName;
              }
              newImg.setAttribute(attrName, attr.value);
            }
          }
          noscript.parentNode.replaceChild(tmp.firstElementChild, prevElement);
        }
      });
    },
    _removeScripts(doc) {
      this._removeNodes(this._getAllNodesWithTag(doc, ["script", "noscript"]));
    },
    _hasSingleTagInsideElement(element, tag) {
      if (element.children.length != 1 || element.children[0].tagName !== tag) {
        return false;
      }
      return !this._someNode(element.childNodes, function(node) {
        return node.nodeType === this.TEXT_NODE && this.REGEXPS.hasContent.test(node.textContent);
      });
    },
    _isElementWithoutContent(node) {
      return node.nodeType === this.ELEMENT_NODE && !node.textContent.trim().length && (!node.children.length || node.children.length == node.getElementsByTagName("br").length + node.getElementsByTagName("hr").length);
    },
    _hasChildBlockElement(element) {
      return this._someNode(element.childNodes, function(node) {
        return this.DIV_TO_P_ELEMS.has(node.tagName) || this._hasChildBlockElement(node);
      });
    },
    _isPhrasingContent(node) {
      return node.nodeType === this.TEXT_NODE || this.PHRASING_ELEMS.includes(node.tagName) || (node.tagName === "A" || node.tagName === "DEL" || node.tagName === "INS") && this._everyNode(node.childNodes, this._isPhrasingContent);
    },
    _isWhitespace(node) {
      return node.nodeType === this.TEXT_NODE && node.textContent.trim().length === 0 || node.nodeType === this.ELEMENT_NODE && node.tagName === "BR";
    },
    _getInnerText(e, normalizeSpaces) {
      normalizeSpaces = typeof normalizeSpaces === "undefined" ? true : normalizeSpaces;
      var textContent = e.textContent.trim();
      if (normalizeSpaces) {
        return textContent.replace(this.REGEXPS.normalize, " ");
      }
      return textContent;
    },
    _getCharCount(e, s) {
      s = s || ",";
      return this._getInnerText(e).split(s).length - 1;
    },
    _cleanStyles(e) {
      if (!e || e.tagName.toLowerCase() === "svg") {
        return;
      }
      for (var i = 0;i < this.PRESENTATIONAL_ATTRIBUTES.length; i++) {
        e.removeAttribute(this.PRESENTATIONAL_ATTRIBUTES[i]);
      }
      if (this.DEPRECATED_SIZE_ATTRIBUTE_ELEMS.includes(e.tagName)) {
        e.removeAttribute("width");
        e.removeAttribute("height");
      }
      var cur = e.firstElementChild;
      while (cur !== null) {
        this._cleanStyles(cur);
        cur = cur.nextElementSibling;
      }
    },
    _getLinkDensity(element) {
      var textLength = this._getInnerText(element).length;
      if (textLength === 0) {
        return 0;
      }
      var linkLength = 0;
      this._forEachNode(element.getElementsByTagName("a"), function(linkNode) {
        var href = linkNode.getAttribute("href");
        var coefficient = href && this.REGEXPS.hashUrl.test(href) ? 0.3 : 1;
        linkLength += this._getInnerText(linkNode).length * coefficient;
      });
      return linkLength / textLength;
    },
    _getClassWeight(e) {
      if (!this._flagIsActive(this.FLAG_WEIGHT_CLASSES)) {
        return 0;
      }
      var weight = 0;
      if (typeof e.className === "string" && e.className !== "") {
        if (this.REGEXPS.negative.test(e.className)) {
          weight -= 25;
        }
        if (this.REGEXPS.positive.test(e.className)) {
          weight += 25;
        }
      }
      if (typeof e.id === "string" && e.id !== "") {
        if (this.REGEXPS.negative.test(e.id)) {
          weight -= 25;
        }
        if (this.REGEXPS.positive.test(e.id)) {
          weight += 25;
        }
      }
      return weight;
    },
    _clean(e, tag) {
      var isEmbed = ["object", "embed", "iframe"].includes(tag);
      this._removeNodes(this._getAllNodesWithTag(e, [tag]), function(element) {
        if (isEmbed) {
          for (var i = 0;i < element.attributes.length; i++) {
            if (this._allowedVideoRegex.test(element.attributes[i].value)) {
              return false;
            }
          }
          if (element.tagName === "object" && this._allowedVideoRegex.test(element.innerHTML)) {
            return false;
          }
        }
        return true;
      });
    },
    _hasAncestorTag(node, tagName, maxDepth, filterFn) {
      maxDepth = maxDepth || 3;
      tagName = tagName.toUpperCase();
      var depth = 0;
      while (node.parentNode) {
        if (maxDepth > 0 && depth > maxDepth) {
          return false;
        }
        if (node.parentNode.tagName === tagName && (!filterFn || filterFn(node.parentNode))) {
          return true;
        }
        node = node.parentNode;
        depth++;
      }
      return false;
    },
    _getRowAndColumnCount(table) {
      var rows = 0;
      var columns = 0;
      var trs = table.getElementsByTagName("tr");
      for (var i = 0;i < trs.length; i++) {
        var rowspan = trs[i].getAttribute("rowspan") || 0;
        if (rowspan) {
          rowspan = parseInt(rowspan, 10);
        }
        rows += rowspan || 1;
        var columnsInThisRow = 0;
        var cells = trs[i].getElementsByTagName("td");
        for (var j = 0;j < cells.length; j++) {
          var colspan = cells[j].getAttribute("colspan") || 0;
          if (colspan) {
            colspan = parseInt(colspan, 10);
          }
          columnsInThisRow += colspan || 1;
        }
        columns = Math.max(columns, columnsInThisRow);
      }
      return { rows, columns };
    },
    _markDataTables(root) {
      var tables = root.getElementsByTagName("table");
      for (var i = 0;i < tables.length; i++) {
        var table = tables[i];
        var role = table.getAttribute("role");
        if (role == "presentation") {
          table._readabilityDataTable = false;
          continue;
        }
        var datatable = table.getAttribute("datatable");
        if (datatable == "0") {
          table._readabilityDataTable = false;
          continue;
        }
        var summary = table.getAttribute("summary");
        if (summary) {
          table._readabilityDataTable = true;
          continue;
        }
        var caption = table.getElementsByTagName("caption")[0];
        if (caption && caption.childNodes.length) {
          table._readabilityDataTable = true;
          continue;
        }
        var dataTableDescendants = ["col", "colgroup", "tfoot", "thead", "th"];
        var descendantExists = function(tag) {
          return !!table.getElementsByTagName(tag)[0];
        };
        if (dataTableDescendants.some(descendantExists)) {
          this.log("Data table because found data-y descendant");
          table._readabilityDataTable = true;
          continue;
        }
        if (table.getElementsByTagName("table")[0]) {
          table._readabilityDataTable = false;
          continue;
        }
        var sizeInfo = this._getRowAndColumnCount(table);
        if (sizeInfo.columns == 1 || sizeInfo.rows == 1) {
          table._readabilityDataTable = false;
          continue;
        }
        if (sizeInfo.rows >= 10 || sizeInfo.columns > 4) {
          table._readabilityDataTable = true;
          continue;
        }
        table._readabilityDataTable = sizeInfo.rows * sizeInfo.columns > 10;
      }
    },
    _fixLazyImages(root) {
      this._forEachNode(this._getAllNodesWithTag(root, ["img", "picture", "figure"]), function(elem) {
        if (elem.src && this.REGEXPS.b64DataUrl.test(elem.src)) {
          var parts = this.REGEXPS.b64DataUrl.exec(elem.src);
          if (parts[1] === "image/svg+xml") {
            return;
          }
          var srcCouldBeRemoved = false;
          for (var i = 0;i < elem.attributes.length; i++) {
            var attr = elem.attributes[i];
            if (attr.name === "src") {
              continue;
            }
            if (/\.(jpg|jpeg|png|webp)/i.test(attr.value)) {
              srcCouldBeRemoved = true;
              break;
            }
          }
          if (srcCouldBeRemoved) {
            var b64starts = parts[0].length;
            var b64length = elem.src.length - b64starts;
            if (b64length < 133) {
              elem.removeAttribute("src");
            }
          }
        }
        if ((elem.src || elem.srcset && elem.srcset != "null") && !elem.className.toLowerCase().includes("lazy")) {
          return;
        }
        for (var j = 0;j < elem.attributes.length; j++) {
          attr = elem.attributes[j];
          if (attr.name === "src" || attr.name === "srcset" || attr.name === "alt") {
            continue;
          }
          var copyTo = null;
          if (/\.(jpg|jpeg|png|webp)\s+\d/.test(attr.value)) {
            copyTo = "srcset";
          } else if (/^\s*\S+\.(jpg|jpeg|png|webp)\S*\s*$/.test(attr.value)) {
            copyTo = "src";
          }
          if (copyTo) {
            if (elem.tagName === "IMG" || elem.tagName === "PICTURE") {
              elem.setAttribute(copyTo, attr.value);
            } else if (elem.tagName === "FIGURE" && !this._getAllNodesWithTag(elem, ["img", "picture"]).length) {
              var img = this._doc.createElement("img");
              img.setAttribute(copyTo, attr.value);
              elem.appendChild(img);
            }
          }
        }
      });
    },
    _getTextDensity(e, tags) {
      var textLength = this._getInnerText(e, true).length;
      if (textLength === 0) {
        return 0;
      }
      var childrenLength = 0;
      var children = this._getAllNodesWithTag(e, tags);
      this._forEachNode(children, (child) => childrenLength += this._getInnerText(child, true).length);
      return childrenLength / textLength;
    },
    _cleanConditionally(e, tag) {
      if (!this._flagIsActive(this.FLAG_CLEAN_CONDITIONALLY)) {
        return;
      }
      this._removeNodes(this._getAllNodesWithTag(e, [tag]), function(node) {
        var isDataTable = function(t) {
          return t._readabilityDataTable;
        };
        var isList = tag === "ul" || tag === "ol";
        if (!isList) {
          var listLength = 0;
          var listNodes = this._getAllNodesWithTag(node, ["ul", "ol"]);
          this._forEachNode(listNodes, (list) => listLength += this._getInnerText(list).length);
          isList = listLength / this._getInnerText(node).length > 0.9;
        }
        if (tag === "table" && isDataTable(node)) {
          return false;
        }
        if (this._hasAncestorTag(node, "table", -1, isDataTable)) {
          return false;
        }
        if (this._hasAncestorTag(node, "code")) {
          return false;
        }
        if ([...node.getElementsByTagName("table")].some((tbl) => tbl._readabilityDataTable)) {
          return false;
        }
        var weight = this._getClassWeight(node);
        this.log("Cleaning Conditionally", node);
        var contentScore = 0;
        if (weight + contentScore < 0) {
          return true;
        }
        if (this._getCharCount(node, ",") < 10) {
          var p = node.getElementsByTagName("p").length;
          var img = node.getElementsByTagName("img").length;
          var li = node.getElementsByTagName("li").length - 100;
          var input = node.getElementsByTagName("input").length;
          var headingDensity = this._getTextDensity(node, [
            "h1",
            "h2",
            "h3",
            "h4",
            "h5",
            "h6"
          ]);
          var embedCount = 0;
          var embeds = this._getAllNodesWithTag(node, [
            "object",
            "embed",
            "iframe"
          ]);
          for (var i = 0;i < embeds.length; i++) {
            for (var j = 0;j < embeds[i].attributes.length; j++) {
              if (this._allowedVideoRegex.test(embeds[i].attributes[j].value)) {
                return false;
              }
            }
            if (embeds[i].tagName === "object" && this._allowedVideoRegex.test(embeds[i].innerHTML)) {
              return false;
            }
            embedCount++;
          }
          var innerText = this._getInnerText(node);
          if (this.REGEXPS.adWords.test(innerText) || this.REGEXPS.loadingWords.test(innerText)) {
            return true;
          }
          var contentLength = innerText.length;
          var linkDensity = this._getLinkDensity(node);
          var textishTags = ["SPAN", "LI", "TD"].concat(Array.from(this.DIV_TO_P_ELEMS));
          var textDensity = this._getTextDensity(node, textishTags);
          var isFigureChild = this._hasAncestorTag(node, "figure");
          const shouldRemoveNode = () => {
            const errs = [];
            if (!isFigureChild && img > 1 && p / img < 0.5) {
              errs.push(`Bad p to img ratio (img=${img}, p=${p})`);
            }
            if (!isList && li > p) {
              errs.push(`Too many li's outside of a list. (li=${li} > p=${p})`);
            }
            if (input > Math.floor(p / 3)) {
              errs.push(`Too many inputs per p. (input=${input}, p=${p})`);
            }
            if (!isList && !isFigureChild && headingDensity < 0.9 && contentLength < 25 && (img === 0 || img > 2) && linkDensity > 0) {
              errs.push(`Suspiciously short. (headingDensity=${headingDensity}, img=${img}, linkDensity=${linkDensity})`);
            }
            if (!isList && weight < 25 && linkDensity > 0.2 + this._linkDensityModifier) {
              errs.push(`Low weight and a little linky. (linkDensity=${linkDensity})`);
            }
            if (weight >= 25 && linkDensity > 0.5 + this._linkDensityModifier) {
              errs.push(`High weight and mostly links. (linkDensity=${linkDensity})`);
            }
            if (embedCount === 1 && contentLength < 75 || embedCount > 1) {
              errs.push(`Suspicious embed. (embedCount=${embedCount}, contentLength=${contentLength})`);
            }
            if (img === 0 && textDensity === 0) {
              errs.push(`No useful content. (img=${img}, textDensity=${textDensity})`);
            }
            if (errs.length) {
              this.log("Checks failed", errs);
              return true;
            }
            return false;
          };
          var haveToRemove = shouldRemoveNode();
          if (isList && haveToRemove) {
            for (var x = 0;x < node.children.length; x++) {
              let child = node.children[x];
              if (child.children.length > 1) {
                return haveToRemove;
              }
            }
            let li_count = node.getElementsByTagName("li").length;
            if (img == li_count) {
              return false;
            }
          }
          return haveToRemove;
        }
        return false;
      });
    },
    _cleanMatchedNodes(e, filter) {
      var endOfSearchMarkerNode = this._getNextNode(e, true);
      var next = this._getNextNode(e);
      while (next && next != endOfSearchMarkerNode) {
        if (filter.call(this, next, next.className + " " + next.id)) {
          next = this._removeAndGetNext(next);
        } else {
          next = this._getNextNode(next);
        }
      }
    },
    _cleanHeaders(e) {
      let headingNodes = this._getAllNodesWithTag(e, ["h1", "h2"]);
      this._removeNodes(headingNodes, function(node) {
        let shouldRemove = this._getClassWeight(node) < 0;
        if (shouldRemove) {
          this.log("Removing header with low class weight:", node);
        }
        return shouldRemove;
      });
    },
    _headerDuplicatesTitle(node) {
      if (node.tagName != "H1" && node.tagName != "H2") {
        return false;
      }
      var heading = this._getInnerText(node, false);
      this.log("Evaluating similarity of header:", heading, this._articleTitle);
      return this._textSimilarity(this._articleTitle, heading) > 0.75;
    },
    _flagIsActive(flag) {
      return (this._flags & flag) > 0;
    },
    _removeFlag(flag) {
      this._flags = this._flags & ~flag;
    },
    _isProbablyVisible(node) {
      return (!node.style || node.style.display != "none") && (!node.style || node.style.visibility != "hidden") && !node.hasAttribute("hidden") && (!node.hasAttribute("aria-hidden") || node.getAttribute("aria-hidden") != "true" || node.className && node.className.includes && node.className.includes("fallback-image"));
    },
    parse() {
      if (this._maxElemsToParse > 0) {
        var numTags = this._doc.getElementsByTagName("*").length;
        if (numTags > this._maxElemsToParse) {
          throw new Error("Aborting parsing document; " + numTags + " elements found");
        }
      }
      this._unwrapNoscriptImages(this._doc);
      var jsonLd = this._disableJSONLD ? {} : this._getJSONLD(this._doc);
      this._removeScripts(this._doc);
      this._prepDocument();
      var metadata = this._getArticleMetadata(jsonLd);
      this._metadata = metadata;
      this._articleTitle = metadata.title;
      var articleContent = this._grabArticle();
      if (!articleContent) {
        return null;
      }
      this.log("Grabbed: " + articleContent.innerHTML);
      this._postProcessContent(articleContent);
      if (!metadata.excerpt) {
        var paragraphs = articleContent.getElementsByTagName("p");
        if (paragraphs.length) {
          metadata.excerpt = paragraphs[0].textContent.trim();
        }
      }
      var textContent = articleContent.textContent;
      return {
        title: this._articleTitle,
        byline: metadata.byline || this._articleByline,
        dir: this._articleDir,
        lang: this._articleLang,
        content: this._serializer(articleContent),
        textContent,
        length: textContent.length,
        excerpt: metadata.excerpt,
        siteName: metadata.siteName || this._articleSiteName,
        publishedTime: metadata.publishedTime
      };
    }
  };
  if (typeof module === "object") {
    module.exports = Readability;
  }
});

// node_modules/@mozilla/readability/Readability-readerable.js
var require_Readability_readerable = __commonJS((exports, module) => {
  var REGEXPS = {
    unlikelyCandidates: /-ad-|ai2html|banner|breadcrumbs|combx|comment|community|cover-wrap|disqus|extra|footer|gdpr|header|legends|menu|related|remark|replies|rss|shoutbox|sidebar|skyscraper|social|sponsor|supplemental|ad-break|agegate|pagination|pager|popup|yom-remote/i,
    okMaybeItsACandidate: /and|article|body|column|content|main|shadow/i
  };
  function isNodeVisible(node) {
    return (!node.style || node.style.display != "none") && !node.hasAttribute("hidden") && (!node.hasAttribute("aria-hidden") || node.getAttribute("aria-hidden") != "true" || node.className && node.className.includes && node.className.includes("fallback-image"));
  }
  function isProbablyReaderable(doc, options = {}) {
    if (typeof options == "function") {
      options = { visibilityChecker: options };
    }
    var defaultOptions = {
      minScore: 20,
      minContentLength: 140,
      visibilityChecker: isNodeVisible
    };
    options = Object.assign(defaultOptions, options);
    var nodes = doc.querySelectorAll("p, pre, article");
    var brNodes = doc.querySelectorAll("div > br");
    if (brNodes.length) {
      var set = new Set(nodes);
      [].forEach.call(brNodes, function(node) {
        set.add(node.parentNode);
      });
      nodes = Array.from(set);
    }
    var score = 0;
    return [].some.call(nodes, function(node) {
      if (!options.visibilityChecker(node)) {
        return false;
      }
      var matchString = node.className + " " + node.id;
      if (REGEXPS.unlikelyCandidates.test(matchString) && !REGEXPS.okMaybeItsACandidate.test(matchString)) {
        return false;
      }
      if (node.matches("li p")) {
        return false;
      }
      var textContentLength = node.textContent.trim().length;
      if (textContentLength < options.minContentLength) {
        return false;
      }
      score += Math.sqrt(textContentLength - options.minContentLength);
      if (score > options.minScore) {
        return true;
      }
      return false;
    });
  }
  if (typeof module === "object") {
    module.exports = isProbablyReaderable;
  }
});

// node_modules/@mozilla/readability/index.js
var require_readability = __commonJS((exports, module) => {
  var Readability = require_Readability();
  var isProbablyReaderable = require_Readability_readerable();
  module.exports = {
    Readability,
    isProbablyReaderable
  };
});

// src/detect.ts
var RULES = [
  { type: "zhihu", test: /^https?:\/\/(?:www\.)?zhihu\.com\/(?:question\/\d+\/answer\/\d+|p\/\d+)/i },
  { type: "youtube-channel", test: /^https?:\/\/(?:www\.)?youtube\.com\/(?:@|channel\/|c\/|user\/)/i },
  { type: "youtube", test: /^https?:\/\/(?:www\.|m\.)?(?:youtube\.com\/(?:watch\?|shorts\/|embed\/)|youtu\.be\/)/i },
  { type: "bilibili", test: /^https?:\/\/(?:www\.|m\.)?bilibili\.com\/video\//i },
  { type: "vimeo", test: /^https?:\/\/(?:www\.)?vimeo\.com\/\d+/i },
  { type: "tiktok", test: /^https?:\/\/(?:www\.)?tiktok\.com\/@[^/]+\/video\/\d+/i },
  { type: "twitter", test: /^https?:\/\/(?:www\.)?(?:twitter\.com|x\.com)\/[^/]+\/status\/\d+/i },
  { type: "bluesky", test: /^https?:\/\/(?:www\.)?bsky\.app\/profile\/[^/]+\/post\/[^/?#]+/i },
  { type: "mastodon", test: /^https?:\/\/[^/]+\/@[^/]+\/\d+/i },
  { type: "stackexchange", test: /^https?:\/\/(?:[a-z0-9-]+\.)?(?:stackexchange|stackoverflow|superuser|serverfault|askubuntu)\.com\/(?:questions|q)\/\d+/i },
  { type: "github", test: /^https?:\/\/(?:www\.)?github\.com\/[^/]+\/[^/]+\/?(?:(?:issues|pull)\/\d+)?$/i },
  { type: "wikipedia", test: /^https?:\/\/[a-z-]+\.wikipedia\.org\/wiki\//i },
  { type: "pinterest", test: /^https?:\/\/(?:www\.)?pinterest\.[a-z.]+\/pin\/\d+/i }
];
function isUrl(text) {
  return /^https?:\/\/\S+$/i.test(text.trim());
}
function detectType(url) {
  const trimmed = url.trim();
  return RULES.find((rule) => rule.test.test(trimmed))?.type ?? "website";
}
function splitBatch(text) {
  return text.split(/[\s,;]+/).map((part) => part.trim()).filter((part) => isUrl(part));
}

// src/parsers/text.ts
function parseText(text) {
  const firstLine = text.trim().split(`
`)[0] ?? "";
  return {
    type: "text",
    title: firstLine.slice(0, 60) || "Clipboard",
    content: `${text}
`,
    description: "",
    extra: { chars: text.length }
  };
}

// src/parsers/website.ts
var import_readability = __toESM(require_readability(), 1);

// node_modules/turndown/lib/turndown.browser.es.js
function extend(destination) {
  for (var i = 1;i < arguments.length; i++) {
    var source = arguments[i];
    for (var key in source) {
      if (Object.prototype.hasOwnProperty.call(source, key))
        destination[key] = source[key];
    }
  }
  return destination;
}
function repeat(character, count) {
  return Array(count + 1).join(character);
}
function trimLeadingNewlines(string) {
  return string.replace(/^\n*/, "");
}
function trimTrailingNewlines(string) {
  var indexEnd = string.length;
  while (indexEnd > 0 && string[indexEnd - 1] === `
`)
    indexEnd--;
  return string.substring(0, indexEnd);
}
function trimNewlines(string) {
  return trimTrailingNewlines(trimLeadingNewlines(string));
}
var blockElements = ["ADDRESS", "ARTICLE", "ASIDE", "AUDIO", "BLOCKQUOTE", "BODY", "CANVAS", "CENTER", "DD", "DIR", "DIV", "DL", "DT", "FIELDSET", "FIGCAPTION", "FIGURE", "FOOTER", "FORM", "FRAMESET", "H1", "H2", "H3", "H4", "H5", "H6", "HEADER", "HGROUP", "HR", "HTML", "ISINDEX", "LI", "MAIN", "MENU", "NAV", "NOFRAMES", "NOSCRIPT", "OL", "OUTPUT", "P", "PRE", "SECTION", "TABLE", "TBODY", "TD", "TFOOT", "TH", "THEAD", "TR", "UL"];
function isBlock(node) {
  return is(node, blockElements);
}
var voidElements = ["AREA", "BASE", "BR", "COL", "COMMAND", "EMBED", "HR", "IMG", "INPUT", "KEYGEN", "LINK", "META", "PARAM", "SOURCE", "TRACK", "WBR"];
function isVoid(node) {
  return is(node, voidElements);
}
function hasVoid(node) {
  return has(node, voidElements);
}
var meaningfulWhenBlankElements = ["A", "TABLE", "THEAD", "TBODY", "TFOOT", "TH", "TD", "IFRAME", "SCRIPT", "AUDIO", "VIDEO"];
function isMeaningfulWhenBlank(node) {
  return is(node, meaningfulWhenBlankElements);
}
function hasMeaningfulWhenBlank(node) {
  return has(node, meaningfulWhenBlankElements);
}
function is(node, tagNames) {
  return tagNames.indexOf(node.nodeName) >= 0;
}
function has(node, tagNames) {
  return node.getElementsByTagName && tagNames.some(function(tagName) {
    return node.getElementsByTagName(tagName).length;
  });
}
var markdownEscapes = [[/\\/g, "\\\\"], [/\*/g, "\\*"], [/^-/g, "\\-"], [/^\+ /g, "\\+ "], [/^(=+)/g, "\\$1"], [/^(#{1,6}) /g, "\\$1 "], [/`/g, "\\`"], [/^~~~/g, "\\~~~"], [/\[/g, "\\["], [/\]/g, "\\]"], [/^>/g, "\\>"], [/_/g, "\\_"], [/^(\d+)\. /g, "$1\\. "]];
function escapeMarkdown(string) {
  return markdownEscapes.reduce(function(accumulator, escape) {
    return accumulator.replace(escape[0], escape[1]);
  }, string);
}
var rules = {};
rules.paragraph = {
  filter: "p",
  replacement: function(content) {
    return `

` + content + `

`;
  }
};
rules.lineBreak = {
  filter: "br",
  replacement: function(content, node, options) {
    return options.br + `
`;
  }
};
rules.heading = {
  filter: ["h1", "h2", "h3", "h4", "h5", "h6"],
  replacement: function(content, node, options) {
    var hLevel = Number(node.nodeName.charAt(1));
    if (options.headingStyle === "setext" && hLevel < 3) {
      var underline = repeat(hLevel === 1 ? "=" : "-", content.length);
      return `

` + content + `
` + underline + `

`;
    } else {
      return `

` + repeat("#", hLevel) + " " + content + `

`;
    }
  }
};
rules.blockquote = {
  filter: "blockquote",
  replacement: function(content) {
    content = trimNewlines(content).replace(/^/gm, "> ");
    return `

` + content + `

`;
  }
};
rules.list = {
  filter: ["ul", "ol"],
  replacement: function(content, node) {
    var parent = node.parentNode;
    if (parent.nodeName === "LI" && parent.lastElementChild === node) {
      return `
` + content;
    } else {
      return `

` + content + `

`;
    }
  }
};
rules.listItem = {
  filter: "li",
  replacement: function(content, node, options) {
    var prefix = options.bulletListMarker + "   ";
    var parent = node.parentNode;
    if (parent.nodeName === "OL") {
      var start = parent.getAttribute("start");
      var index = Array.prototype.indexOf.call(parent.children, node);
      prefix = (start ? Number(start) + index : index + 1) + ".  ";
    }
    var isParagraph = /\n$/.test(content);
    content = trimNewlines(content) + (isParagraph ? `
` : "");
    content = content.replace(/\n/gm, `
` + " ".repeat(prefix.length));
    return prefix + content + (node.nextSibling ? `
` : "");
  }
};
rules.indentedCodeBlock = {
  filter: function(node, options) {
    return options.codeBlockStyle === "indented" && node.nodeName === "PRE" && node.firstChild && node.firstChild.nodeName === "CODE";
  },
  replacement: function(content, node, options) {
    return `

    ` + node.firstChild.textContent.replace(/\n/g, `
    `) + `

`;
  }
};
rules.fencedCodeBlock = {
  filter: function(node, options) {
    return options.codeBlockStyle === "fenced" && node.nodeName === "PRE" && node.firstChild && node.firstChild.nodeName === "CODE";
  },
  replacement: function(content, node, options) {
    var className = node.firstChild.getAttribute("class") || "";
    var language = (className.match(/language-(\S+)/) || [null, ""])[1];
    var code = node.firstChild.textContent;
    var fenceChar = options.fence.charAt(0);
    var fenceSize = 3;
    var fenceInCodeRegex = new RegExp("^" + fenceChar + "{3,}", "gm");
    var match;
    while (match = fenceInCodeRegex.exec(code)) {
      if (match[0].length >= fenceSize) {
        fenceSize = match[0].length + 1;
      }
    }
    var fence = repeat(fenceChar, fenceSize);
    return `

` + fence + language + `
` + code.replace(/\n$/, "") + `
` + fence + `

`;
  }
};
rules.horizontalRule = {
  filter: "hr",
  replacement: function(content, node, options) {
    return `

` + options.hr + `

`;
  }
};
rules.inlineLink = {
  filter: function(node, options) {
    return options.linkStyle === "inlined" && node.nodeName === "A" && node.getAttribute("href");
  },
  replacement: function(content, node) {
    var href = escapeLinkDestination(node.getAttribute("href"));
    var title = escapeLinkTitle(cleanAttribute(node.getAttribute("title")));
    var titlePart = title ? ' "' + title + '"' : "";
    return "[" + content + "](" + href + titlePart + ")";
  }
};
rules.referenceLink = {
  filter: function(node, options) {
    return options.linkStyle === "referenced" && node.nodeName === "A" && node.getAttribute("href");
  },
  replacement: function(content, node, options) {
    var href = escapeLinkDestination(node.getAttribute("href"));
    var title = cleanAttribute(node.getAttribute("title"));
    if (title)
      title = ' "' + escapeLinkTitle(title) + '"';
    var replacement;
    var reference;
    switch (options.linkReferenceStyle) {
      case "collapsed":
        replacement = "[" + content + "][]";
        reference = "[" + content + "]: " + href + title;
        break;
      case "shortcut":
        replacement = "[" + content + "]";
        reference = "[" + content + "]: " + href + title;
        break;
      default:
        var id = this.references.length + 1;
        replacement = "[" + content + "][" + id + "]";
        reference = "[" + id + "]: " + href + title;
    }
    this.references.push(reference);
    return replacement;
  },
  references: [],
  append: function(options) {
    var references = "";
    if (this.references.length) {
      references = `

` + this.references.join(`
`) + `

`;
      this.references = [];
    }
    return references;
  }
};
rules.emphasis = {
  filter: ["em", "i"],
  replacement: function(content, node, options) {
    if (!content.trim())
      return "";
    return options.emDelimiter + content + options.emDelimiter;
  }
};
rules.strong = {
  filter: ["strong", "b"],
  replacement: function(content, node, options) {
    if (!content.trim())
      return "";
    return options.strongDelimiter + content + options.strongDelimiter;
  }
};
rules.code = {
  filter: function(node) {
    var hasSiblings = node.previousSibling || node.nextSibling;
    var isCodeBlock = node.parentNode.nodeName === "PRE" && !hasSiblings;
    return node.nodeName === "CODE" && !isCodeBlock;
  },
  replacement: function(content) {
    if (!content)
      return "";
    content = content.replace(/\r?\n|\r/g, " ");
    var extraSpace = /^`|^ .*?[^ ].* $|`$/.test(content) ? " " : "";
    var delimiter = "`";
    var matches = content.match(/`+/gm) || [];
    while (matches.indexOf(delimiter) !== -1)
      delimiter = delimiter + "`";
    return delimiter + extraSpace + content + extraSpace + delimiter;
  }
};
rules.image = {
  filter: "img",
  replacement: function(content, node) {
    var alt = escapeMarkdown(cleanAttribute(node.getAttribute("alt")));
    var src = escapeLinkDestination(node.getAttribute("src") || "");
    var title = cleanAttribute(node.getAttribute("title"));
    var titlePart = title ? ' "' + escapeLinkTitle(title) + '"' : "";
    return src ? "![" + alt + "]" + "(" + src + titlePart + ")" : "";
  }
};
function cleanAttribute(attribute) {
  return attribute ? attribute.replace(/(\n+\s*)+/g, `
`) : "";
}
function escapeLinkDestination(destination) {
  var escaped = destination.replace(/([<>()])/g, "\\$1");
  return escaped.indexOf(" ") >= 0 ? "<" + escaped + ">" : escaped;
}
function escapeLinkTitle(title) {
  return title.replace(/"/g, "\\\"");
}
function Rules(options) {
  this.options = options;
  this._keep = [];
  this._remove = [];
  this.blankRule = {
    replacement: options.blankReplacement
  };
  this.keepReplacement = options.keepReplacement;
  this.defaultRule = {
    replacement: options.defaultReplacement
  };
  this.array = [];
  for (var key in options.rules)
    this.array.push(options.rules[key]);
}
Rules.prototype = {
  add: function(key, rule) {
    this.array.unshift(rule);
  },
  keep: function(filter) {
    this._keep.unshift({
      filter,
      replacement: this.keepReplacement
    });
  },
  remove: function(filter) {
    this._remove.unshift({
      filter,
      replacement: function() {
        return "";
      }
    });
  },
  forNode: function(node) {
    if (node.isBlank)
      return this.blankRule;
    var rule;
    if (rule = findRule(this.array, node, this.options))
      return rule;
    if (rule = findRule(this._keep, node, this.options))
      return rule;
    if (rule = findRule(this._remove, node, this.options))
      return rule;
    return this.defaultRule;
  },
  forEach: function(fn) {
    for (var i = 0;i < this.array.length; i++)
      fn(this.array[i], i);
  }
};
function findRule(rules2, node, options) {
  for (var i = 0;i < rules2.length; i++) {
    var rule = rules2[i];
    if (filterValue(rule, node, options))
      return rule;
  }
  return;
}
function filterValue(rule, node, options) {
  var filter = rule.filter;
  if (typeof filter === "string") {
    if (filter === node.nodeName.toLowerCase())
      return true;
  } else if (Array.isArray(filter)) {
    if (filter.indexOf(node.nodeName.toLowerCase()) > -1)
      return true;
  } else if (typeof filter === "function") {
    if (filter.call(rule, node, options))
      return true;
  } else {
    throw new TypeError("`filter` needs to be a string, array, or function");
  }
}
function collapseWhitespace(options) {
  var element = options.element;
  var isBlock2 = options.isBlock;
  var isVoid2 = options.isVoid;
  var isPre = options.isPre || function(node2) {
    return node2.nodeName === "PRE";
  };
  if (!element.firstChild || isPre(element))
    return;
  var prevText = null;
  var keepLeadingWs = false;
  var prev = null;
  var node = next(prev, element, isPre);
  while (node !== element) {
    if (node.nodeType === 3 || node.nodeType === 4) {
      var text = node.data.replace(/[ \r\n\t]+/g, " ");
      if ((!prevText || / $/.test(prevText.data)) && !keepLeadingWs && text[0] === " ") {
        text = text.substr(1);
      }
      if (!text) {
        node = remove(node);
        continue;
      }
      node.data = text;
      prevText = node;
    } else if (node.nodeType === 1) {
      if (isBlock2(node) || node.nodeName === "BR") {
        if (prevText) {
          prevText.data = prevText.data.replace(/ $/, "");
        }
        prevText = null;
        keepLeadingWs = false;
      } else if (isVoid2(node) || isPre(node)) {
        prevText = null;
        keepLeadingWs = true;
      } else if (prevText) {
        keepLeadingWs = false;
      }
    } else {
      node = remove(node);
      continue;
    }
    var nextNode = next(prev, node, isPre);
    prev = node;
    node = nextNode;
  }
  if (prevText) {
    prevText.data = prevText.data.replace(/ $/, "");
    if (!prevText.data) {
      remove(prevText);
    }
  }
}
function remove(node) {
  var next = node.nextSibling || node.parentNode;
  node.parentNode.removeChild(node);
  return next;
}
function next(prev, current, isPre) {
  if (prev && prev.parentNode === current || isPre(current)) {
    return current.nextSibling || current.parentNode;
  }
  return current.firstChild || current.nextSibling || current.parentNode;
}
var root = typeof window !== "undefined" ? window : {};
function canParseHTMLNatively() {
  var Parser = root.DOMParser;
  var canParse = false;
  try {
    if (new Parser().parseFromString("", "text/html")) {
      canParse = true;
    }
  } catch (e) {}
  return canParse;
}
function createHTMLParser() {
  var Parser = function() {};
  {
    if (shouldUseActiveX()) {
      Parser.prototype.parseFromString = function(string) {
        var doc = new window.ActiveXObject("htmlfile");
        doc.designMode = "on";
        doc.open();
        doc.write(string);
        doc.close();
        return doc;
      };
    } else {
      Parser.prototype.parseFromString = function(string) {
        var doc = document.implementation.createHTMLDocument("");
        doc.open();
        doc.write(string);
        doc.close();
        return doc;
      };
    }
  }
  return Parser;
}
function shouldUseActiveX() {
  var useActiveX = false;
  try {
    document.implementation.createHTMLDocument("").open();
  } catch (e) {
    if (root.ActiveXObject)
      useActiveX = true;
  }
  return useActiveX;
}
var HTMLParser = canParseHTMLNatively() ? root.DOMParser : createHTMLParser();
function RootNode(input, options) {
  var root2;
  if (typeof input === "string") {
    var doc = htmlParser().parseFromString('<x-turndown id="turndown-root">' + input + "</x-turndown>", "text/html");
    root2 = doc.getElementById("turndown-root");
  } else {
    root2 = input.cloneNode(true);
  }
  collapseWhitespace({
    element: root2,
    isBlock,
    isVoid,
    isPre: options.preformattedCode ? isPreOrCode : null
  });
  return root2;
}
var _htmlParser;
function htmlParser() {
  _htmlParser = _htmlParser || new HTMLParser;
  return _htmlParser;
}
function isPreOrCode(node) {
  return node.nodeName === "PRE" || node.nodeName === "CODE";
}
function Node(node, options) {
  node.isBlock = isBlock(node);
  node.isCode = node.nodeName === "CODE" || node.parentNode.isCode;
  node.isBlank = isBlank(node);
  node.flankingWhitespace = flankingWhitespace(node, options);
  return node;
}
function isBlank(node) {
  return !isVoid(node) && !isMeaningfulWhenBlank(node) && /^\s*$/i.test(node.textContent) && !hasVoid(node) && !hasMeaningfulWhenBlank(node);
}
function flankingWhitespace(node, options) {
  if (node.isBlock || options.preformattedCode && node.isCode) {
    return {
      leading: "",
      trailing: ""
    };
  }
  var edges = edgeWhitespace(node.textContent);
  if (edges.leadingAscii && isFlankedByWhitespace("left", node, options)) {
    edges.leading = edges.leadingNonAscii;
  }
  if (edges.trailingAscii && isFlankedByWhitespace("right", node, options)) {
    edges.trailing = edges.trailingNonAscii;
  }
  return {
    leading: edges.leading,
    trailing: edges.trailing
  };
}
function edgeWhitespace(string) {
  var m = string.match(/^(([ \t\r\n]*)(\s*))(?:(?=\S)[\s\S]*\S)?((\s*?)([ \t\r\n]*))$/);
  return {
    leading: m[1],
    leadingAscii: m[2],
    leadingNonAscii: m[3],
    trailing: m[4],
    trailingNonAscii: m[5],
    trailingAscii: m[6]
  };
}
function isFlankedByWhitespace(side, node, options) {
  var sibling;
  var regExp;
  var isFlanked;
  if (side === "left") {
    sibling = node.previousSibling;
    regExp = / $/;
  } else {
    sibling = node.nextSibling;
    regExp = /^ /;
  }
  if (sibling) {
    if (sibling.nodeType === 3) {
      isFlanked = regExp.test(sibling.nodeValue);
    } else if (options.preformattedCode && sibling.nodeName === "CODE") {
      isFlanked = false;
    } else if (sibling.nodeType === 1 && !isBlock(sibling)) {
      isFlanked = regExp.test(sibling.textContent);
    }
  }
  return isFlanked;
}
var reduce = Array.prototype.reduce;
function TurndownService(options) {
  if (!(this instanceof TurndownService))
    return new TurndownService(options);
  var defaults = {
    rules,
    headingStyle: "setext",
    hr: "* * *",
    bulletListMarker: "*",
    codeBlockStyle: "indented",
    fence: "```",
    emDelimiter: "_",
    strongDelimiter: "**",
    linkStyle: "inlined",
    linkReferenceStyle: "full",
    br: "  ",
    preformattedCode: false,
    blankReplacement: function(content, node) {
      return node.isBlock ? `

` : "";
    },
    keepReplacement: function(content, node) {
      return node.isBlock ? `

` + node.outerHTML + `

` : node.outerHTML;
    },
    defaultReplacement: function(content, node) {
      return node.isBlock ? `

` + content + `

` : content;
    }
  };
  this.options = extend({}, defaults, options);
  this.rules = new Rules(this.options);
}
TurndownService.prototype = {
  turndown: function(input) {
    if (!canConvert(input)) {
      throw new TypeError(input + " is not a string, or an element/document/fragment node.");
    }
    if (input === "")
      return "";
    var output = process.call(this, new RootNode(input, this.options));
    return postProcess.call(this, output);
  },
  use: function(plugin) {
    if (Array.isArray(plugin)) {
      for (var i = 0;i < plugin.length; i++)
        this.use(plugin[i]);
    } else if (typeof plugin === "function") {
      plugin(this);
    } else {
      throw new TypeError("plugin must be a Function or an Array of Functions");
    }
    return this;
  },
  addRule: function(key, rule) {
    this.rules.add(key, rule);
    return this;
  },
  keep: function(filter) {
    this.rules.keep(filter);
    return this;
  },
  remove: function(filter) {
    this.rules.remove(filter);
    return this;
  },
  escape: function(string) {
    return escapeMarkdown(string);
  }
};
function process(parentNode) {
  var self = this;
  return reduce.call(parentNode.childNodes, function(output, node) {
    node = new Node(node, self.options);
    var replacement = "";
    if (node.nodeType === 3) {
      replacement = node.isCode ? node.nodeValue : self.escape(node.nodeValue);
    } else if (node.nodeType === 1) {
      replacement = replacementForNode.call(self, node);
    }
    join(output, replacement);
    return output;
  }, []);
}
function postProcess(output) {
  var self = this;
  this.rules.forEach(function(rule) {
    if (typeof rule.append === "function") {
      join(output, rule.append(self.options));
    }
  });
  return output.join("").replace(/^[\t\r\n]+/, "").replace(/[\t\r\n\s]+$/, "");
}
function replacementForNode(node) {
  var rule = this.rules.forNode(node);
  var content = process.call(this, node).join("");
  var whitespace = node.flankingWhitespace;
  if (whitespace.leading || whitespace.trailing)
    content = content.trim();
  return whitespace.leading + rule.replacement(content, node, this.options) + whitespace.trailing;
}
function join(output, replacement) {
  var tail = output.pop() || "";
  var s1 = trimTrailingNewlines(tail);
  var s2 = trimLeadingNewlines(replacement);
  var nls = Math.max(tail.length - s1.length, replacement.length - s2.length);
  var separator = `

`.substring(0, nls);
  if (s1)
    output.push(s1);
  if (separator)
    output.push(separator);
  if (s2)
    output.push(s2);
}
function canConvert(input) {
  return input != null && (typeof input === "string" || input.nodeType && (input.nodeType === 1 || input.nodeType === 9 || input.nodeType === 11));
}

// node_modules/turndown-plugin-gfm/lib/turndown-plugin-gfm.es.js
var highlightRegExp = /highlight-(?:text|source)-([a-z0-9]+)/;
function highlightedCodeBlock(turndownService) {
  turndownService.addRule("highlightedCodeBlock", {
    filter: function(node) {
      var firstChild = node.firstChild;
      return node.nodeName === "DIV" && highlightRegExp.test(node.className) && firstChild && firstChild.nodeName === "PRE";
    },
    replacement: function(content, node, options) {
      var className = node.className || "";
      var language = (className.match(highlightRegExp) || [null, ""])[1];
      return `

` + options.fence + language + `
` + node.firstChild.textContent + `
` + options.fence + `

`;
    }
  });
}
function strikethrough(turndownService) {
  turndownService.addRule("strikethrough", {
    filter: ["del", "s", "strike"],
    replacement: function(content) {
      return "~" + content + "~";
    }
  });
}
var indexOf = Array.prototype.indexOf;
var every = Array.prototype.every;
var rules2 = {};
rules2.tableCell = {
  filter: ["th", "td"],
  replacement: function(content, node) {
    return cell(content, node);
  }
};
rules2.tableRow = {
  filter: "tr",
  replacement: function(content, node) {
    var borderCells = "";
    var alignMap = { left: ":--", right: "--:", center: ":-:" };
    if (isHeadingRow(node)) {
      for (var i = 0;i < node.childNodes.length; i++) {
        var border = "---";
        var align = (node.childNodes[i].getAttribute("align") || "").toLowerCase();
        if (align)
          border = alignMap[align] || border;
        borderCells += cell(border, node.childNodes[i]);
      }
    }
    return `
` + content + (borderCells ? `
` + borderCells : "");
  }
};
rules2.table = {
  filter: function(node) {
    return node.nodeName === "TABLE" && isHeadingRow(node.rows[0]);
  },
  replacement: function(content) {
    content = content.replace(`

`, `
`);
    return `

` + content + `

`;
  }
};
rules2.tableSection = {
  filter: ["thead", "tbody", "tfoot"],
  replacement: function(content) {
    return content;
  }
};
function isHeadingRow(tr) {
  var parentNode = tr.parentNode;
  return parentNode.nodeName === "THEAD" || parentNode.firstChild === tr && (parentNode.nodeName === "TABLE" || isFirstTbody(parentNode)) && every.call(tr.childNodes, function(n) {
    return n.nodeName === "TH";
  });
}
function isFirstTbody(element) {
  var previousSibling = element.previousSibling;
  return element.nodeName === "TBODY" && (!previousSibling || previousSibling.nodeName === "THEAD" && /^\s*$/i.test(previousSibling.textContent));
}
function cell(content, node) {
  var index = indexOf.call(node.parentNode.childNodes, node);
  var prefix = " ";
  if (index === 0)
    prefix = "| ";
  return prefix + content + " |";
}
function tables(turndownService) {
  turndownService.keep(function(node) {
    return node.nodeName === "TABLE" && !isHeadingRow(node.rows[0]);
  });
  for (var key in rules2)
    turndownService.addRule(key, rules2[key]);
}
function taskListItems(turndownService) {
  turndownService.addRule("taskListItems", {
    filter: function(node) {
      return node.type === "checkbox" && node.parentNode.nodeName === "LI";
    },
    replacement: function(content, node) {
      return (node.checked ? "[x]" : "[ ]") + " ";
    }
  });
}
function gfm(turndownService) {
  turndownService.use([
    highlightedCodeBlock,
    strikethrough,
    tables,
    taskListItems
  ]);
}

// src/parsers/website.ts
var MAX_IMAGES = 40;
function turndown() {
  const service = new TurndownService({ headingStyle: "atx", codeBlockStyle: "fenced", hr: "---" });
  service.use(gfm);
  service.remove(["script", "style", "noscript", "iframe"]);
  return service;
}
function absolute(url, base) {
  try {
    return new URL(url, base).toString();
  } catch {
    return null;
  }
}
function base64ToBytes(base64) {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0;i < binary.length; i++)
    bytes[i] = binary.charCodeAt(i);
  return bytes;
}
async function rewriteImages(html, pageUrl, ctx) {
  const doc = new DOMParser().parseFromString(`<!doctype html><html><body>${html}</body></html>`, "text/html");
  const sources = Array.from(doc.querySelectorAll("img[src]")).slice(0, MAX_IMAGES);
  let images = 0;
  for (const [index, img] of sources.entries()) {
    const original = absolute(img.getAttribute("src") ?? "", pageUrl);
    if (!original)
      continue;
    try {
      const binary = await ctx.proxyBinary(original);
      const extension = (binary.contentType.split("/")[1] ?? "bin").split(";")[0];
      const stored = await ctx.upload(base64ToBytes(binary.base64), `clip-${Date.now()}-${index}.${extension}`, binary.contentType);
      img.setAttribute("src", stored);
      images += 1;
    } catch {
      img.setAttribute("src", original);
    }
  }
  for (const anchor of Array.from(doc.querySelectorAll("a[href]"))) {
    const href = anchor.getAttribute("href") ?? "";
    if (href.startsWith("#") || /^[a-z][a-z0-9+.-]*:/i.test(href))
      continue;
    const absoluteHref = absolute(href, pageUrl);
    if (absoluteHref)
      anchor.setAttribute("href", absoluteHref);
  }
  return { html: doc.body.innerHTML, images };
}
async function parseWebsite(url, ctx) {
  const response = await ctx.proxy(url);
  if (response.status >= 400) {
    throw new Error(`page answered ${response.status}`);
  }
  const document2 = new DOMParser().parseFromString(response.body, "text/html");
  const article = new import_readability.Readability(document2, { charThreshold: 120 }).parse();
  if (!article?.content) {
    return {
      type: "website",
      title: document2.title || url,
      content: `[${document2.title || url}](${url})
`,
      url,
      description: "",
      images: 0
    };
  }
  const { html, images } = await rewriteImages(article.content, url, ctx);
  const markdown = turndown().turndown(html);
  return {
    type: "website",
    title: (article.title || document2.title || url).trim(),
    content: markdown,
    description: (article.excerpt ?? "").slice(0, 240),
    author: article.byline ?? undefined,
    published: article.publishedTime ?? undefined,
    url,
    tags: [],
    images,
    extra: { wordCount: article.length ?? 0 }
  };
}

// src/parsers/oembed.ts
var CONFIGS = [
  {
    type: "youtube",
    endpoint: (url) => `https://www.youtube.com/oembed?format=json&url=${encodeURIComponent(url)}`,
    embed: (url) => {
      const id = url.match(/(?:v=|youtu\.be\/|shorts\/|embed\/)([\w-]{6,})/)?.[1] ?? "";
      return `<iframe width="100%" height="420" src="https://www.youtube-nocookie.com/embed/${id}" title="YouTube" frameborder="0" allowfullscreen></iframe>`;
    }
  },
  {
    type: "bilibili",
    endpoint: (url) => `https://www.bilibili.com/oembed?format=json&url=${encodeURIComponent(url)}`,
    embed: (url) => {
      const id = url.match(/\/video\/(BV[\w]+)/)?.[1] ?? "";
      return `<iframe width="100%" height="420" src="https://player.bilibili.com/player.html?bvid=${id}" frameborder="0" allowfullscreen></iframe>`;
    }
  },
  {
    type: "vimeo",
    endpoint: (url) => `https://vimeo.com/api/oembed.json?url=${encodeURIComponent(url)}`,
    embed: (url) => {
      const id = url.match(/vimeo\.com\/(\d+)/)?.[1] ?? "";
      return `<iframe width="100%" height="420" src="https://player.vimeo.com/video/${id}" frameborder="0" allowfullscreen></iframe>`;
    }
  },
  {
    type: "tiktok",
    endpoint: (url) => `https://www.tiktok.com/oembed?url=${encodeURIComponent(url)}`,
    embed: (url) => `[Watch on TikTok](${url})`
  }
];
async function parseOEmbed(url, ctx) {
  const config = CONFIGS.find((candidate) => candidate.endpoint(url));
  if (!config)
    return null;
  const endpoint = config.endpoint(url);
  let meta = {};
  try {
    const response = await ctx.proxy(endpoint);
    if (response.status < 400)
      meta = JSON.parse(response.body);
  } catch {}
  const title = meta.title || url;
  return {
    type: config.type,
    title,
    content: `${config.embed(url)}
`,
    description: meta.author_name ? `by ${meta.author_name}` : "",
    author: meta.author_name,
    url,
    tags: [],
    extra: { provider: meta.provider_name ?? config.type }
  };
}

// src/parsers/api.ts
function decodeEntities(text) {
  return text.replace(/&quot;/g, '"').replace(/&#39;/g, "'").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&amp;/g, "&");
}
async function parseStackExchange(url, ctx) {
  const match = url.match(/^https?:\/\/([a-z0-9-]+)\.(?:stackexchange|com)\/questions\/(\d+)/i);
  if (!match)
    return null;
  const [, site, id] = match;
  const host = url.includes("stackoverflow") ? "stackoverflow" : url.includes("superuser") ? "superuser" : url.includes("serverfault") ? "serverfault" : url.includes("askubuntu") ? "askubuntu" : site;
  const api = `https://api.stackexchange.com/2.3/questions/${id}?site=${host}&filter=withbody`;
  const answersApi = `https://api.stackexchange.com/2.3/questions/${id}/answers?site=${host}&filter=withbody&sort=votes&order=desc&pagesize=5`;
  const [question, answers] = await Promise.all([ctx.proxy(api), ctx.proxy(answersApi)]);
  if (question.status >= 400)
    return null;
  const parsed = JSON.parse(question.body);
  const item = parsed.items?.[0];
  if (!item)
    return null;
  const answerBody = (() => {
    try {
      const list = JSON.parse(answers.body);
      const best = list.items?.[0];
      return best ? `
## Best answer

${decodeEntities(String(best.body ?? ""))}
` : "";
    } catch {
      return "";
    }
  })();
  return {
    type: "stackexchange",
    title: decodeEntities(String(item.title ?? url)),
    content: `${decodeEntities(String(item.body ?? ""))}
${answerBody}`,
    description: `${String(item.answer_count ?? 0)} answers · score ${String(item.score ?? 0)}`,
    url,
    tags: Array.isArray(item.tags) ? item.tags : [],
    extra: { score: item.score ?? 0, answers: item.answer_count ?? 0 }
  };
}
async function parseBluesky(url, ctx) {
  const match = url.match(/bsky\.app\/profile\/([^/]+)\/post\/([^/?#]+)/i);
  if (!match)
    return null;
  const [, handle, rkey] = match;
  const atUri = `at://${handle}/app.bsky.feed.post/${rkey}`;
  const api = `https://public.api.bsky.app/xrpc/app.bsky.feed.getPostThread?uri=${encodeURIComponent(atUri)}&depth=0`;
  const response = await ctx.proxy(api);
  if (response.status >= 400)
    return null;
  const data = JSON.parse(response.body);
  const post = data.thread?.post;
  if (!post?.record?.text)
    return null;
  const images = post.embed?.images ?? [];
  const imageMarkdown = images.map((image) => image.fullsize).filter((src) => Boolean(src)).map((src) => `![](${src})`).join(`
`);
  return {
    type: "bluesky",
    title: `${post.author?.displayName || post.author?.handle || handle}: ${post.record.text.slice(0, 60)}`,
    content: `${post.record.text}

${imageMarkdown}
`,
    description: `${post.likeCount ?? 0} likes`,
    author: post.author?.displayName || post.author?.handle,
    published: post.record.createdAt,
    url,
    extra: { likes: post.likeCount ?? 0 }
  };
}
async function parseMastodon(url, ctx) {
  const match = url.match(/^(https?:\/\/[^/]+)\/@([^/]+)\/(\d+)/i);
  if (!match)
    return null;
  const [, instance, , id] = match;
  const api = `${instance}/api/v1/statuses/${id}`;
  const response = await ctx.proxy(api);
  if (response.status >= 400)
    return null;
  const status = JSON.parse(response.body);
  if (!status.content)
    return null;
  const media = (status.media_attachments ?? []).map((attachment) => attachment.url).filter((src) => Boolean(src)).map((src) => `![](${src})`).join(`
`);
  return {
    type: "mastodon",
    title: `${status.account?.display_name || status.account?.acct || "Mastodon"}: ${decodeEntities(status.content).slice(0, 60)}`,
    content: `${status.content}

${media}
`,
    description: `${status.favourites_count ?? 0} favourites`,
    author: status.account?.display_name || status.account?.acct,
    published: status.created_at,
    url,
    extra: { favourites: status.favourites_count ?? 0 }
  };
}
async function parseGithub(url, ctx) {
  const match = url.match(/github\.com\/([^/]+)\/([^/]+)(?:\/(issues|pull)\/(\d+))?/i);
  if (!match)
    return null;
  const [, owner, repo, kind, number] = match;
  const api = kind && number ? `https://api.github.com/repos/${owner}/${repo}/issues/${number}` : `https://api.github.com/repos/${owner}/${repo}`;
  const response = await ctx.proxy(api, { headers: { accept: "application/vnd.github+json", "user-agent": "pergame-clipper" } });
  if (response.status >= 400)
    return null;
  const data = JSON.parse(response.body);
  if (kind && number) {
    return {
      type: "github",
      title: `${owner}/${repo}#${number} ${String(data.title ?? "")}`.trim(),
      content: `${String(data.body ?? "")}
`,
      description: `${String(data.state ?? "")} · ${String(data.comments ?? 0)} comments`,
      author: data.user?.login,
      published: String(data.created_at ?? ""),
      url,
      tags: Array.isArray(data.labels) ? data.labels.map((label) => label.name ?? "").filter(Boolean) : [],
      extra: { state: data.state ?? "", comments: data.comments ?? 0 }
    };
  }
  return {
    type: "github",
    title: `${owner}/${repo}`,
    content: `${String(data.description ?? "")}

${String(data.html_url ?? url)}
`,
    description: `${String(data.language ?? "")} · ★ ${String(data.stargazers_count ?? 0)}`,
    author: owner,
    url,
    tags: Array.isArray(data.topics) ? data.topics : [],
    extra: { stars: data.stargazers_count ?? 0, forks: data.forks_count ?? 0 }
  };
}
async function parseWikipedia(url, ctx) {
  const match = url.match(/^https?:\/\/([a-z-]+)\.wikipedia\.org\/wiki\/(.+)$/i);
  if (!match)
    return null;
  const [, lang, title] = match;
  const api = `https://${lang}.wikipedia.org/api/rest_v1/page/summary/${encodeURIComponent(decodeURIComponent(title))}`;
  const response = await ctx.proxy(api);
  if (response.status >= 400)
    return null;
  const data = JSON.parse(response.body);
  if (!data.extract)
    return null;
  return {
    type: "wikipedia",
    title: data.title ?? url,
    content: `${data.extract}

[Read on Wikipedia](${url})
`,
    description: data.description ?? "",
    url,
    extra: {}
  };
}

// src/parsers/zhihu.ts
var ZHIHU_COOKIE_KEY = "zhihu_cookie";
function toMarkdown(html) {
  const service = new TurndownService({ headingStyle: "atx", codeBlockStyle: "fenced" });
  service.use(gfm);
  service.remove(["script", "style", "noscript"]);
  return service.turndown(html);
}
async function parseZhihu(url, ctx) {
  const cookie = await ctx.setting(ZHIHU_COOKIE_KEY);
  const response = await ctx.proxy(url, {
    headers: {
      "user-agent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126 Safari/537.36",
      "accept-language": "zh-CN,zh;q=0.9",
      ...cookie ? { cookie } : {}
    }
  });
  if (response.status >= 400) {
    throw new Error(`Zhihu answered ${response.status}${cookie ? "" : " (no cookie configured)"}`);
  }
  const document2 = new DOMParser().parseFromString(response.body, "text/html");
  const isArticle = /\/p\/\d+/.test(url);
  const title = isArticle ? document2.querySelector(".Post-Title")?.textContent?.trim() ?? document2.title : document2.querySelector(".QuestionHeader-title")?.textContent?.trim() ?? document2.title;
  const body = isArticle ? document2.querySelector(".Post-RichText")?.innerHTML : document2.querySelector(".RichText")?.innerHTML ?? document2.querySelector(".AnswerItem .RichText")?.innerHTML;
  if (!body) {
    throw new Error("Zhihu returned a challenge page; paste the article instead or add a cookie in plugin settings");
  }
  const author = document2.querySelector(".AuthorInfo-name")?.textContent?.trim() ?? undefined;
  return {
    type: "zhihu",
    title: title ?? url,
    content: toMarkdown(body),
    description: isArticle ? "Zhihu column" : "Zhihu answer",
    author,
    url,
    tags: ["zhihu"],
    extra: {}
  };
}

// src/template.ts
var VARIABLE_RE = /\{\{(.*?)\}\}/g;
var FOR_RE = /\{%\s*for\s+(\w+)\s+in\s+([\w.]+)\s*%\}([\s\S]*?)\{%\s*endfor\s*%\}/g;
function lookup(data, path) {
  return path.split(".").reduce((value, key) => {
    if (value && typeof value === "object" && key in value) {
      return value[key];
    }
    return;
  }, data);
}
function lexify(value) {
  const abs = Math.abs(value);
  if (abs >= 1e9)
    return `${(value / 1e9).toFixed(1)}B`;
  if (abs >= 1e6)
    return `${(value / 1e6).toFixed(1)}M`;
  if (abs >= 1000)
    return `${(value / 1000).toFixed(1)}K`;
  return String(value);
}
function parseExpression(expression) {
  const [rawPath, ...rawFilters] = expression.split("|").map((part) => part.trim());
  const filters = rawFilters.map((part) => {
    const match = part.match(/^(\w+)(?:\((.*)\))?$/);
    if (!match)
      return { name: part, args: [] };
    const args = (match[2] ?? "").split(",").map((arg) => arg.trim()).filter(Boolean).map((arg) => arg.replace(/^["']|["']$/g, ""));
    return { name: match[1], args };
  });
  return { path: rawPath ?? "", filters };
}
function applyFilter(name, value, args) {
  const text = value == null ? "" : String(value);
  switch (name) {
    case "blockquote":
      return text.split(`
`).map((line) => `> ${line}`).join(`
`);
    case "capitalize":
      return text.charAt(0).toUpperCase() + text.slice(1);
    case "upper":
      return text.toUpperCase();
    case "lower":
      return text.toLowerCase();
    case "replace":
      return text.split(args[0] ?? "").join(args[1] ?? "");
    case "join":
      return Array.isArray(value) ? value.join(args[0] ?? ", ") : text;
    case "striptags":
      return text.replace(/<[^>]*>/g, "");
    case "map":
      return Array.isArray(value) ? value.map((item) => item && typeof item === "object" ? item[args[0] ?? ""] : item) : value;
    case "numberLexify":
      return typeof value === "number" ? lexify(value) : text;
    case "default":
      return text || (args[0] ?? "");
    default:
      return value;
  }
}
function renderExpression(expression, data) {
  const { path, filters } = parseExpression(expression);
  let value = lookup(data, path);
  for (const filter of filters)
    value = applyFilter(filter.name, value, filter.args);
  return value == null ? "" : String(value);
}
function renderTemplate(template, data) {
  const withLoops = template.replace(FOR_RE, (_match, itemName, listPath, body) => {
    const list = lookup(data, listPath);
    if (!Array.isArray(list))
      return "";
    return list.map((item) => {
      const scoped = { ...data, [itemName]: item };
      return body.replace(VARIABLE_RE, (_m, expression) => renderExpression(expression, scoped));
    }).join("");
  });
  return withLoops.replace(VARIABLE_RE, (_match, expression) => renderExpression(expression, data));
}

// src/service.ts
var SETTINGS = {
  parentId: "parent_id",
  tags: "default_tags",
  templatePrefix: "template_",
  zhihuCookie: "zhihu_cookie"
};
var DEFAULT_TEMPLATES = {
  website: `> {{ description }}

[Source]({{ url }})

{{ content }}
`,
  zhihu: `[Source]({{ url }}){{ author | default("") }}

{{ content }}
`,
  youtube: `{{ content }}

[Watch on YouTube]({{ url }})
`,
  "youtube-channel": `{{ content }}

[Channel]({{ url }})
`,
  bilibili: `{{ content }}

[Watch on Bilibili]({{ url }})
`,
  vimeo: `{{ content }}

[Watch on Vimeo]({{ url }})
`,
  tiktok: `{{ content }}

[Watch on TikTok]({{ url }})
`,
  twitter: `[Source]({{ url }})

{{ content }}
`,
  bluesky: `[Source]({{ url }})

{{ content }}
`,
  mastodon: `[Source]({{ url }})

{{ content }}
`,
  stackexchange: `[Source]({{ url }})

{{ content }}
`,
  github: `[Source]({{ url }})

{{ content }}
`,
  wikipedia: `{{ content }}
`,
  pinterest: `[Source]({{ url }})

{{ content }}
`,
  text: `{{ content }}
`
};
async function parseByType(url, type, ctx) {
  switch (type) {
    case "zhihu":
      return await parseZhihu(url, ctx) ?? parseWebsite(url, ctx);
    case "youtube":
    case "bilibili":
    case "vimeo":
    case "tiktok": {
      const embedded = await parseOEmbed(url, ctx);
      return embedded ?? parseWebsite(url, ctx);
    }
    case "stackexchange":
      return await parseStackExchange(url, ctx) ?? parseWebsite(url, ctx);
    case "bluesky":
      return await parseBluesky(url, ctx) ?? parseWebsite(url, ctx);
    case "mastodon":
      return await parseMastodon(url, ctx) ?? parseWebsite(url, ctx);
    case "github":
      return await parseGithub(url, ctx) ?? parseWebsite(url, ctx);
    case "wikipedia":
      return await parseWikipedia(url, ctx) ?? parseWebsite(url, ctx);
    default:
      return parseWebsite(url, ctx);
  }
}
async function clip(host, input, overrides = {}) {
  const { ctx } = host;
  const trimmed = input.trim();
  if (!trimmed)
    throw new Error("nothing to clip");
  const type = isUrl(trimmed) ? detectType(trimmed) : "text";
  const parsed = isUrl(trimmed) ? await parseByType(trimmed, type, ctx) : parseText(trimmed);
  const templateKey = `${SETTINGS.templatePrefix}${parsed.type}`;
  const template = await ctx.setting(templateKey) || DEFAULT_TEMPLATES[parsed.type] || DEFAULT_TEMPLATES.website;
  const body = renderTemplate(template, {
    ...parsed.extra,
    title: parsed.title,
    content: parsed.content,
    description: parsed.description ?? "",
    author: parsed.author ?? "",
    published: parsed.published ?? "",
    url: parsed.url ?? "",
    tags: parsed.tags ?? []
  });
  const parentId = overrides.parentId || await ctx.setting(SETTINGS.parentId) || undefined;
  const tags = overrides.tags ?? await ctx.setting(SETTINGS.tags) ?? "";
  const name = (overrides.title || parsed.title || "Clipped note").slice(0, 60);
  const note = await host.createNote({
    name,
    content: body,
    parent_id: parentId,
    tags: tags || undefined
  });
  host.notify(`Clipped ${name}`, { type: "success", timeout: 4000 });
  return { type: parsed.type, title: name, noteId: note.id, images: parsed.images ?? 0 };
}
async function clipBatch(host, text, overrides = {}) {
  const urls = splitBatch(text);
  if (!urls.length) {
    host.notify("No URLs in the clipboard", { type: "warning" });
    return { ok: 0, failed: 0 };
  }
  let ok = 0;
  let failed = 0;
  for (const url of urls) {
    try {
      await clip(host, url, overrides);
      ok += 1;
    } catch (err) {
      failed += 1;
      const reason = err instanceof Error ? err.message : String(err);
      host.notify(`Clip failed: ${url} — ${reason}`, { type: "error", timeout: 6000 });
    }
  }
  host.notify(`Clipped ${ok}/${urls.length}${failed ? ` (${failed} failed)` : ""}`, { type: failed ? "warning" : "success" });
  return { ok, failed };
}

// src/main.ts
class ReadItLater {
  host;
  ctxRef;
  async onload(ctx) {
    const folders = async () => {
      const top = [{ value: "", label: "Top level" }];
      const listing = ctx.nodes.listChildren().then((nodes) => nodes.filter((node) => node.role === 1 || node.role === 2).map((node) => ({ value: node.id, label: node.name }))).catch(() => []);
      const containers = await Promise.race([
        listing,
        new Promise((resolve) => setTimeout(() => resolve([]), 1500))
      ]);
      return [...top, ...containers];
    };
    const parserCtx = {
      proxy: (url, init) => ctx.net.proxy(url, init),
      proxyBinary: (url) => ctx.net.proxyBinary(url),
      upload: async (bytes, filename, contentType) => {
        const blob = new Blob([bytes], { type: contentType });
        const stored = await ctx.resources.upload(new File([blob], filename, { type: contentType }));
        return `/api/resources/${stored.id}/file`;
      },
      setting: (key) => ctx.settings.get(key)
    };
    const host = {
      ctx: parserCtx,
      createNote: (input) => ctx.nodes.create({ ...input, role: 3 }),
      notify: (message, opts) => ctx.ui.notice(message, opts)
    };
    this.host = host;
    this.ctxRef = ctx;
    const askAndClip = async (initialUrl) => {
      const saved = await ctx.settings.get(SETTINGS.parentId);
      const result = await ctx.ui.modal({
        title: "Clip to note",
        description: "Paste a URL, or any text to save it as a note. Leave the title empty to use the page title.",
        fields: [
          { key: "url", label: "URL or text", type: "textarea", value: initialUrl, required: true },
          { key: "title", label: "Title (optional)", type: "text", placeholder: "Use the detected title" },
          { key: "tags", label: "Tags (optional)", type: "text", placeholder: "comma, separated" },
          { key: "parent", label: "Folder", type: "select", value: saved ?? "", choices: await folders() }
        ],
        submitLabel: "Clip"
      });
      if (!result)
        return;
      try {
        await clip(host, String(result.url ?? ""), {
          parentId: String(result.parent ?? "") || undefined,
          tags: String(result.tags ?? ""),
          title: String(result.title ?? "") || undefined
        });
      } catch (err) {
        ctx.ui.notice(`Clip failed: ${err instanceof Error ? err.message : String(err)}`, { type: "error", timeout: 8000 });
      }
    };
    ctx.ui.command("clip", "Read it later: clip a URL", async () => {
      const clipboard = await navigator.clipboard.readText().catch(() => "");
      await askAndClip(clipboard.trim());
    });
    ctx.ui.command("clip-clipboard", "Read it later: clip from clipboard", async () => {
      const text = await navigator.clipboard.readText().catch(() => "");
      if (!text.trim()) {
        ctx.ui.notice("Clipboard is empty", { type: "warning" });
        return;
      }
      try {
        await clip(host, text);
      } catch (err) {
        ctx.ui.notice(`Clip failed: ${err instanceof Error ? err.message : String(err)}`, { type: "error", timeout: 8000 });
      }
    });
    ctx.ui.command("clip-batch", "Read it later: clip every URL in the clipboard", async () => {
      const text = await navigator.clipboard.readText().catch(() => "");
      await clipBatch(host, text);
    });
    ctx.ui.ribbon("\uD83D\uDDDE️", "Read it later", async () => {
      const text = await navigator.clipboard.readText().catch(() => "");
      if (text.trim())
        await askAndClip(text.trim());
      else
        await askAndClip("");
    });
    await ctx.settings.set("template_defaults", DEFAULT_TEMPLATES);
  }
  async onunload() {
    this.ctxRef;
  }
}
export {
  ReadItLater as default
};
