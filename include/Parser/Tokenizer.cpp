#include "Tokenizer.h"
#include <cctype>
#include <iostream>

bool Tokenizer::isAlpha(char c) {
    return std::isalpha(static_cast<unsigned char>(c)) || c == '/';
}

bool Tokenizer::isSpace(char c) {
    return std::isspace(static_cast<unsigned char>(c));
}

std::vector<Token> Tokenizer::tokenize(const std::string& html) {
    std::vector<Token> tokens;
    size_t i = 0;
    while (i < html.size()) {
        // 1. If we see "<", we might be entering a tag or comment
        if (html[i] == '<') {
            // Check if it's a comment: <!-- ... -->
            if ((i + 4 < html.size()) && html.compare(i, 4, "<!--") == 0) {
                // parse comment
                size_t endPos = html.find("-->", i+4);
                if (endPos == std::string::npos) {
                    // No closing, treat rest as comment
                    endPos = html.size() - 1;
                }
                Token commentToken;
                commentToken.type = TokenType::COMMENT;
                commentToken.textContent = 
                    html.substr(i+4, endPos - (i+4));
                
                tokens.push_back(commentToken);
                
                // Move index
                i = (endPos + 3 < html.size()) ? endPos + 3 : html.size();
            }
            else {
                // parse tag
                size_t tagClose = html.find('>', i+1);
                if (tagClose == std::string::npos) {
                    // Malformed HTML: no closing '>' found
                    break;
                }
                
                std::string insideTag = 
                    html.substr(i+1, tagClose - (i+1)); 
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
                
                // parse out tagname and attributes
                Token token;
                if (isEndTag) {
                    token.type = TokenType::END_TAG;
                }
                else if (selfClosing) {
                    token.type = TokenType::SELF_CLOSING_TAG;
                } 
                else {
                    token.type = TokenType::START_TAG;
                }
                
                // Trim spaces
                while (!insideTag.empty() && isSpace(insideTag.front()))
                    insideTag.erase(0,1);
                while (!insideTag.empty() && isSpace(insideTag.back()))
                    insideTag.pop_back();

                // separate tagname from attributes
                // find first space
                size_t spacePos = insideTag.find(' ');
                std::string tagName = insideTag;
                std::string attrs;
                if (spacePos != std::string::npos) {
                    tagName = insideTag.substr(0, spacePos);
                    attrs = insideTag.substr(spacePos+1);
                }

                token.tagName = tagName;

                // parse attributes from 'attrs' string
                // naive approach: split on spaces and '='
                // In reality, HTML attributes can be more complicated
                // We'll do a simple approach
                while (!attrs.empty()) {
                    // skip leading spaces
                    while (!attrs.empty() && isSpace(attrs.front()))
                        attrs.erase(0,1);
                    if (attrs.empty()) break;

                    // find '='
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
                    attrs.erase(0, eqPos+1);
                    // skip possible quotes
                    if (!attrs.empty() && (attrs.front() == '"' || attrs.front() == '\''))
                    {
                        char quoteChar = attrs.front();
                        attrs.erase(0,1); // remove the quote
                        // find the matching quote
                        size_t endQuote = attrs.find(quoteChar);
                        std::string attrVal;
                        if (endQuote == std::string::npos) {
                            // no matching quote, take rest
                            attrVal = attrs;
                            attrs.clear();
                        } else {
                            attrVal = attrs.substr(0, endQuote);
                            attrs.erase(0, endQuote+1); // remove the quote
                        }
                        token.attributes[attrName] = attrVal;
                    } 
                    else {
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

                tokens.push_back(token);
                i = tagClose + 1; // move past '>'
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
            } 
            else {
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
