#ifndef HTML_PARSER_H
#define HTML_PARSER_H

#include <string>
#include "DOMNode.h" // Will define this later

class HtmlParser {
public:
    HtmlParser();
    ~HtmlParser();

    DOMNode* parse(const std::string& html); // Parses HTML into a DOM tree
};

#endif // HTML_PARSER_H
