#include "HtmlParser.h"
#include "Tokenizer.h"
#include <stack>
#include <iostream>
#include <vector>
#include <set>
// A partial set of void elements for your parser
static const std::set<std::string> voidElements = {
    "area", "base", "br", "col", "embed", "hr",
    "img", "input", "link", "meta", "param",
    "source", "track", "wbr"
};

DOMNode* HtmlParser::parse(const std::string& html) {
    Tokenizer tokenizer;
    std::vector<Token> tokens = tokenizer.tokenize(html);
    return buildDOM(tokens);
}

DOMNode* HtmlParser::buildDOM(const std::vector<Token>& tokens) {
    DOMNode* root = new DOMNode(NodeType::DOCUMENT);
    root->tagName = "document";

    std::stack<DOMNode*> nodeStack;
    nodeStack.push(root);

    // default
    mode = ParserMode::NORMAL;

    for (size_t i = 0; i < tokens.size(); i++) {
        Token token = tokens[i];

        switch (token.type) {
        case TokenType::START_TAG:
            handleStartTag(token, nodeStack);
            break;
        case TokenType::END_TAG:
            handleEndTag(token, nodeStack);
            break;
        case TokenType::SELF_CLOSING_TAG:
            handleSelfClosingTag(token, nodeStack);
            break;
        case TokenType::COMMENT:
        case TokenType::TEXT:
            handleTextOrComment(token, nodeStack);
            break;
        case TokenType::DOCTYPE:
            // optional: store docType info or ignore
            break;
        }
    }

    // auto-close leftover
    while (nodeStack.size() > 1) {
        std::cerr << "[Renesting] Auto-closing unclosed <"
                  << nodeStack.top()->tagName << ">\n";
        nodeStack.pop();
    }

    return root;
}

void HtmlParser::handleStartTag(Token& token, std::stack<DOMNode*>& nodeStack) {
    // Create new element node
    DOMNode* elem = new DOMNode(NodeType::ELEMENT);
    elem->tagName = token.tagName;
    elem->attributes = token.attributes;

    // attach to current
    nodeStack.top()->appendChild(elem);

    // if it's a void element, do not push
    if (voidElements.count(token.tagName)) {
        return; // no children
    }

    // check special tags
    if (token.tagName == "script") {
        mode = ParserMode::IN_SCRIPT;
    }
    else if (token.tagName == "style") {
        mode = ParserMode::IN_STYLE;
    }

    nodeStack.push(elem);
}

void HtmlParser::handleEndTag(Token& token, std::stack<DOMNode*>& nodeStack) {
    std::string closeTag = token.tagName;

    // handle special modes
    if (mode == ParserMode::IN_SCRIPT && closeTag == "script") {
        mode = ParserMode::NORMAL;
    }
    else if (mode == ParserMode::IN_STYLE && closeTag == "style") {
        mode = ParserMode::NORMAL;
    }

    bool foundMatch = false;
    while (!nodeStack.empty()) {
        DOMNode* top = nodeStack.top();
        if (top->tagName == closeTag) {
            // found
            nodeStack.pop();
            foundMatch = true;
            break;
        } else {
            std::cerr << "[Renesting] Mismatched end tag </" 
                      << closeTag << ">, auto-closing <" 
                      << top->tagName << ">" << std::endl;
            nodeStack.pop();
        }
    }
    if (!foundMatch) {
        std::cerr << "[Renesting] Orphan end tag </" << closeTag 
                  << ">, ignoring.\n";
    }
}

void HtmlParser::handleSelfClosingTag(Token& token, std::stack<DOMNode*>& nodeStack) {
    DOMNode* elem = new DOMNode(NodeType::ELEMENT);
    elem->tagName = token.tagName;
    elem->attributes = token.attributes;

    nodeStack.top()->appendChild(elem);
    // e.g. <img />, <br />, <meta> etc.
    // no push
}

void HtmlParser::handleTextOrComment(Token& token, std::stack<DOMNode*>& nodeStack) {
    // If in IN_SCRIPT or IN_STYLE, treat everything as text except for recognized end tag
    // For robust fix, you'd expand tokenizer logic.
    DOMNode* node;
    if (token.type == TokenType::COMMENT) {
        node = new DOMNode(NodeType::COMMENT);
        node->textContent = token.textContent;
    } else {
        // TEXT
        node = new DOMNode(NodeType::TEXT);
        node->textContent = token.textContent;
    }
    nodeStack.top()->appendChild(node);
}
