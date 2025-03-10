#include "Tokenizer.h"
#include <cctype>
#include <algorithm> // for std::transform
#include <iostream>

bool Tokenizer::isAlpha(char c) {
    return std::isalpha(static_cast<unsigned char>(c)) || c == '/';
}

bool Tokenizer::isSpace(char c) {
    return std::isspace(static_cast<unsigned char>(c));
}

// Helper: lowercase a string
static std::string toLowerCase(const std::string& s) {
    std::string result = s;
    std::transform(result.begin(), result.end(), result.begin(),
                   [](unsigned char c){ return std::tolower(c); });
    return result;
}

// Helper: find case-insensitive substring
static size_t findIgnoreCase(const std::string& data, const std::string& needle, size_t pos=0) {
    // Convert both to lower
    std::string dataLower = toLowerCase(data);
    std::string needleLower = toLowerCase(needle);
    return dataLower.find(needleLower, pos);
}

std::vector<Token> Tokenizer::tokenize(const std::string& html) {
    std::vector<Token> tokens;
    size_t i = 0;

    while (i < html.size()) {
        // If we see "<", might be entering a tag or comment
        if (html[i] == '<') {
            // Check if it's a comment: <!-- ... -->
            if ((i + 4 < html.size()) && html.compare(i, 4, "<!--") == 0) {
                // parse comment
                size_t endPos = html.find("-->", i + 4);
                if (endPos == std::string::npos) {
                    // No closing, treat rest as comment
                    endPos = html.size() - 1;
                }
                Token commentToken;
                commentToken.type = TokenType::COMMENT;
                commentToken.textContent = html.substr(i + 4, endPos - (i + 4));
                
                tokens.push_back(commentToken);
                
                // move index
                i = (endPos + 3 < html.size()) ? endPos + 3 : html.size();
            }
            else {
                // parse normal tag or special raw-text tags
                size_t tagClose = html.find('>', i + 1);
                if (tagClose == std::string::npos) {
                    // Malformed HTML: no closing '>' found
                    break;
                }
                
                std::string insideTag = html.substr(i + 1, tagClose - (i + 1));
                // e.g. "div id=\"main\""
                
                // Distinguish start tag vs. end tag vs. self-closing
                bool isEndTag = false;
                bool selfClosing = false;

                // check if starts with '/'
                if (!insideTag.empty() && insideTag[0] == '/') {
                    isEndTag = true;
                    insideTag.erase(0,1); // remove '/'
                }

                // check if ends with '/'
                if (!insideTag.empty() && insideTag.back() == '/') {
                    selfClosing = true;
                    insideTag.pop_back(); // remove '/'
                }

                // Trim spaces
                while (!insideTag.empty() && isSpace(insideTag.front())) {
                    insideTag.erase(0,1);
                }
                while (!insideTag.empty() && isSpace(insideTag.back())) {
                    insideTag.pop_back();
                }

                // separate tagName from attributes
                size_t spacePos = insideTag.find(' ');
                std::string tagName = insideTag;
                std::string attrs;
                if (spacePos != std::string::npos) {
                    tagName = insideTag.substr(0, spacePos);
                    attrs = insideTag.substr(spacePos + 1);
                }

                // Convert tagName to lowercase for easier comparison
                std::string lowerTagName = toLowerCase(tagName);

                Token token;
                if (isEndTag) {
                    token.type = TokenType::END_TAG;
                } else if (selfClosing) {
                    token.type = TokenType::SELF_CLOSING_TAG;
                } else {
                    token.type = TokenType::START_TAG;
                }

                token.tagName = lowerTagName; // store in lowercase to unify

                // parse attributes (naive)
                while (!attrs.empty()) {
                    // skip leading spaces
                    while (!attrs.empty() && isSpace(attrs.front())) {
                        attrs.erase(0,1);
                    }
                    if (attrs.empty()) break;

                    size_t eqPos = attrs.find('=');
                    if (eqPos == std::string::npos) {
                        // no more attr=val pairs
                        break;
                    }
                    std::string attrName = attrs.substr(0, eqPos);

                    // trim
                    while (!attrName.empty() && isSpace(attrName.back()))
                        attrName.pop_back();

                    // skip '='
                    attrs.erase(0, eqPos + 1);

                    // skip possible quotes
                    if (!attrs.empty() && (attrs.front() == '"' || attrs.front() == '\'')) {
                        char quoteChar = attrs.front();
                        attrs.erase(0,1); // remove the quote
                        size_t endQuote = attrs.find(quoteChar);
                        std::string attrVal;
                        if (endQuote == std::string::npos) {
                            // no matching quote, take rest
                            attrVal = attrs;
                            attrs.clear();
                        } else {
                            attrVal = attrs.substr(0, endQuote);
                            attrs.erase(0, endQuote + 1); // remove the quote
                        }
                        token.attributes[attrName] = attrVal;
                    } else {
                        // unquoted
                        size_t space2 = attrs.find(' ');
                        std::string attrVal;
                        if (space2 == std::string::npos) {
                            attrVal = attrs;
                            attrs.clear();
                        } else {
                            attrVal = attrs.substr(0, space2);
                            attrs.erase(0, space2);
                        }
                        token.attributes[attrName] = attrVal;
                    }
                }

                // push the token for the start/end/selfClosing
                tokens.push_back(token);

                i = tagClose + 1; // move past '>'

                // ------------- RAW TEXT MODE (Script / Style) -------------
                // If we just encountered a START_TAG for "script" or "style", let's parse raw text
                if (!isEndTag && !selfClosing) {
                    // We check if lowerTagName is "script" or "style"
                    if (lowerTagName == "script" || lowerTagName == "style") {
                        // We'll read everything until we find the matching end tag </script> or </style>
                        std::string endTagToFind = "</" + lowerTagName + ">";
                        // do a case-insensitive search from i forward
                        size_t rawClosePos = findIgnoreCase(html, endTagToFind, i);
                        if (rawClosePos == std::string::npos) {
                            // no closing tag found => treat the rest of the doc as text
                            Token textT;
                            textT.type = TokenType::TEXT;
                            textT.textContent = html.substr(i);
                            tokens.push_back(textT);
                            i = html.size(); // done
                        } else {
                            // everything from i to rawClosePos is the raw text
                            Token textT;
                            textT.type = TokenType::TEXT;
                            textT.textContent = html.substr(i, rawClosePos - i);
                            tokens.push_back(textT);

                            // skip the found end tag
                            size_t endLen = endTagToFind.size(); // e.g. 9 for "</script>"
                            i = rawClosePos + endLen;

                            // we also want to parse that end tag as a token
                            Token scriptEnd;
                            scriptEnd.type = TokenType::END_TAG;
                            scriptEnd.tagName = lowerTagName; // "script" or "style"
                            tokens.push_back(scriptEnd);
                        }
                    }
                }
                // ---------------------------------------------------------
            }
        }
        else {
            // TEXT content until next '<'
            size_t nextTag = html.find('<', i);
            if (nextTag == std::string::npos) {
                // rest is text
                std::string text = html.substr(i);
                if (!text.empty()) {
                    Token t;
                    t.type = TokenType::TEXT;
                    t.textContent = text;
                    tokens.push_back(t);
                }
                break;
            } else {
                if (nextTag > i) {
                    std::string text = html.substr(i, nextTag - i);
                    if (!text.empty()) {
                        Token t;
                        t.type = TokenType::TEXT;
                        t.textContent = text;
                        tokens.push_back(t);
                    }
                }
                i = nextTag;
            }
        }
    }
    return tokens;
}
