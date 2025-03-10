#include <iostream>
#include "Network/HttpClient.h"
#include "Parser/HtmlParser.h"
#include "Parser/DOMNode.h"

// Recursive function to print DOM tree
void printDOM(DOMNode* node, int depth = 0) {
    if (!node) return;

    std::string indent(depth * 2, ' ');

    switch (node->nodeType) {
    case NodeType::DOCUMENT:
        std::cout << indent << "[Document]" << std::endl;
        break;

    case NodeType::ELEMENT:
        std::cout << indent << "<" << node->tagName;

        // print attributes if any
        for (const auto& attr : node->attributes) {
            std::cout << " " << attr.first << "=\"" << attr.second << "\"";
        }
        std::cout << ">" << std::endl;
        break;

    case NodeType::TEXT:
        std::cout << indent << "Text: \"" << node->textContent << "\"" << std::endl;
        break;

    case NodeType::COMMENT:
        std::cout << indent << "<!-- " << node->textContent << " -->" << std::endl;
        break;
    }

    // Print children recursively
    for (DOMNode* child : node->children) {
        printDOM(child, depth + 1);
    }
}

int main() {
    HttpClient client;

    // Path to CA certificates (adjust if needed!)
    client.setCAFile("C:\\msys64\\usr\\ssl\\certs\\ca-bundle.crt");

    // Fetch Google's homepage
    std::string html = client.fetch("https://www.google.com");

    // Parse HTML into DOM tree
    HtmlParser parser;
    DOMNode* root = parser.parse(html);

    // Print DOM tree structure
    printDOM(root);

    // Cleanup allocated DOM tree memory
    delete root;

    return 0;
}
