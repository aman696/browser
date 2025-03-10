#ifndef DOM_NODE_H
#define DOM_NODE_H

#include <string>
#include <vector>
#include <map>

enum class NodeType {
    ELEMENT,
    TEXT,
    COMMENT,
    DOCUMENT // optional
};

class DOMNode {
public:
    NodeType nodeType;
    std::string tagName;                     // For ELEMENT nodes
    std::map<std::string, std::string> attributes; // For ELEMENT nodes
    std::string textContent;                 // For TEXT or COMMENT nodes

    std::vector<DOMNode*> children;
    DOMNode* parent = nullptr;

    // convenience constructor
    DOMNode(NodeType t) : nodeType(t) {}
    ~DOMNode() {
        // Clean up children
        for (auto child : children) {
            delete child;
        }
        children.clear();
    }

    // Add child
    void appendChild(DOMNode* child) {
        child->parent = this;
        children.push_back(child);
    }
};

#endif // DOM_NODE_H
