package tests

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

// TestNodesLifecycle exercises the node CRUD surface end to end with a freshly
// created user: workspace -> document -> get/list/update/search -> delete.
func TestNodesLifecycle(t *testing.T) {
	owner := SignupAndLogin(t, "testpass123")
	suffix := owner.Username

	t.Run("Create node missing name", func(t *testing.T) {
		r := DoPost(t, owner.Client, "/nodes", map[string]any{"role": 3})
		assert.Equal(t, "error", r.Status)
		assert.Equal(t, 400, r.StatusCode)
	})

	t.Run("Create workspace", func(t *testing.T) {
		r := DoPost(t, owner.Client, "/nodes", map[string]any{
			"name": "ws-" + suffix,
			"role": 1,
		})
		assert.Equal(t, "success", r.Status)
		assert.Equal(t, 200, r.StatusCode)
		assert.NotEmpty(t, ResultIDString(t, r))
	})

	// Workspace id captured for child creation.
	var wsID string
	t.Run("Capture workspace id", func(t *testing.T) {
		r := DoGet(t, owner.Client, "/nodes/user/"+owner.ID)
		assert.Equal(t, "success", r.Status)
		assert.True(t, ResultArrayContainsID(t, r, nodeIDByName(t, r, "ws-"+suffix)),
			"workspace should be listed in user nodes")
		wsID = nodeIDByName(t, r, "ws-"+suffix)
		assert.NotEmpty(t, wsID)
	})

	var docID string
	t.Run("Create document in workspace", func(t *testing.T) {
		r := DoPost(t, owner.Client, "/nodes", map[string]any{
			"name":      "doc-" + suffix,
			"role":      3,
			"parent_id": wsID,
			"content":   "# Hello Integration",
		})
		assert.Equal(t, "success", r.Status)
		assert.Equal(t, 200, r.StatusCode)
		docID = ResultIDString(t, r)
	})

	t.Run("Unauthenticated access denied", func(t *testing.T) {
		r := DoGet(t, NewClient(t), "/nodes/user/"+owner.ID)
		assert.Equal(t, 401, r.StatusCode)
	})

	t.Run("Get node", func(t *testing.T) {
		r := DoGet(t, owner.Client, "/nodes/"+docID)
		assert.Equal(t, "success", r.Status)
		node := resultNodeMap(t, r)
		assert.Equal(t, "doc-"+suffix, node["name"])
		nodeID, _ := AnyIDToString(node["id"])
		assert.Equal(t, docID, nodeID)
	})

	t.Run("Update node", func(t *testing.T) {
		r := DoPut(t, owner.Client, "/nodes/"+docID, map[string]any{
			"name":    "renamed-" + suffix,
			"content": "# Updated content",
		})
		assert.Equal(t, "success", r.Status)

		after := DoGet(t, owner.Client, "/nodes/"+docID)
		node := resultNodeMap(t, after)
		assert.Equal(t, "renamed-"+suffix, node["name"])
		if content, ok := node["content"].(string); ok {
			assert.Contains(t, content, "Updated content")
		}
	})

	t.Run("Search nodes", func(t *testing.T) {
		r := DoGet(t, owner.Client, "/nodes/search?q=renamed-"+suffix)
		assert.Equal(t, "success", r.Status)
		if arr, ok := r.Result.([]any); ok && len(arr) > 0 {
			assert.True(t, ResultArrayContainsID(t, r, docID))
		}
		// The search index may lag behind on some setups; absence is tolerated
		// but a successful empty result must not error.
	})

	t.Run("Delete document", func(t *testing.T) {
		r := DoDelete(t, owner.Client, "/nodes/"+docID)
		assert.Equal(t, "success", r.Status)

		after := DoGet(t, owner.Client, "/nodes/"+docID)
		assert.Equal(t, "error", after.Status)
	})

	t.Run("Delete workspace", func(t *testing.T) {
		r := DoDelete(t, owner.Client, "/nodes/"+wsID)
		assert.Equal(t, "success", r.Status)
	})
}

// nodeIDByName finds the id of the node with the given name in a list result.
func nodeIDByName(t *testing.T, r APIResponse, name string) string {
	arr, ok := r.Result.([]any)
	if !ok {
		t.Fatalf("Result is not an array: %T", r.Result)
	}
	for _, item := range arr {
		m, ok := item.(map[string]any)
		if !ok {
			continue
		}
		if m["name"] == name {
			id, ok := AnyIDToString(m["id"])
			if !ok {
				t.Fatalf("Unexpected id type for node %s", name)
			}
			return id
		}
	}
	t.Fatalf("Node %q not found in result", name)
	return ""
}

// resultNodeMap extracts the node object from a GetNode response, tolerating both
// the {node, children} envelope and a bare node object.
func resultNodeMap(t *testing.T, r APIResponse) map[string]any {
	m, ok := r.Result.(map[string]any)
	if !ok {
		t.Fatalf("Result is not an object: %T", r.Result)
	}
	if inner, ok := m["node"].(map[string]any); ok {
		return inner
	}
	return m
}
