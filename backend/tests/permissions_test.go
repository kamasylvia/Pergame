package tests

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

// TestNodePermissions covers the sharing surface: direct permissions
// (read -> write escalation, revocation) and the invitation join flow.
func TestNodePermissions(t *testing.T) {
	owner := SignupAndLogin(t, "testpass123")
	collab := SignupAndLogin(t, "testpass123")

	r := DoPost(t, owner.Client, "/nodes", map[string]any{
		"name":    "shared-doc-" + owner.Username,
		"role":    3,
		"content": "# Shared",
	})
	assert.Equal(t, "success", r.Status)
	docID := ResultIDString(t, r)

	t.Run("Collaborator denied without permission", func(t *testing.T) {
		r := DoGet(t, collab.Client, "/nodes/"+docID)
		assert.Equal(t, "error", r.Status)
	})

	var permID string
	t.Run("Owner grants read permission", func(t *testing.T) {
		r := DoPost(t, owner.Client, "/nodes/"+docID+"/permissions", map[string]any{
			"user_id":    collab.ID,
			"permission": 1,
		})
		assert.Equal(t, "success", r.Status)
		permID = ResultIDString(t, r)
	})

	t.Run("Read permission allows get", func(t *testing.T) {
		r := DoGet(t, collab.Client, "/nodes/"+docID)
		assert.Equal(t, "success", r.Status)
	})

	t.Run("Read permission forbids update", func(t *testing.T) {
		r := DoPut(t, collab.Client, "/nodes/"+docID, map[string]any{"name": "hijack"})
		assert.Equal(t, "error", r.Status)
	})

	t.Run("Owner escalates to write", func(t *testing.T) {
		r := DoPatch(t, owner.Client, "/nodes/"+docID+"/permissions/"+permID, map[string]any{
			"permission": 2,
		})
		assert.Equal(t, "success", r.Status)
	})

	t.Run("Write permission allows update", func(t *testing.T) {
		r := DoPut(t, collab.Client, "/nodes/"+docID, map[string]any{
			"name": "collab-edited-" + collab.Username,
		})
		assert.Equal(t, "success", r.Status)
	})

	t.Run("Owner lists permissions", func(t *testing.T) {
		r := DoGet(t, owner.Client, "/nodes/"+docID+"/permissions")
		assert.Equal(t, "success", r.Status)
		arr, ok := r.Result.([]any)
		assert.True(t, ok)
		found := false
		for _, item := range arr {
			if m, ok := item.(map[string]any); ok {
				if uid, ok := AnyIDToString(m["user_id"]); ok && uid == collab.ID {
					found = true
				}
			}
		}
		assert.True(t, found, "collaborator permission should be listed")
	})

	t.Run("Owner revokes permission", func(t *testing.T) {
		r := DoDelete(t, owner.Client, "/nodes/"+docID+"/permissions/"+permID)
		assert.Equal(t, "success", r.Status)

		after := DoGet(t, collab.Client, "/nodes/"+docID)
		assert.Equal(t, "error", after.Status)
	})

	var invitationCode string
	t.Run("Owner creates invitation", func(t *testing.T) {
		r := DoPost(t, owner.Client, "/nodes/"+docID+"/invitations", map[string]any{
			"permission_level": 2,
		})
		assert.Equal(t, "success", r.Status)
		m, ok := r.Result.(map[string]any)
		assert.True(t, ok)
		code, ok := m["invitation_code"].(string)
		assert.True(t, ok, "invitation_code should be a string")
		invitationCode = code
	})

	t.Run("Collaborator joins via invitation", func(t *testing.T) {
		r := DoPost(t, collab.Client, "/nodes/invitations/join", map[string]any{
			"code": invitationCode,
		})
		assert.Equal(t, "success", r.Status)
	})

	t.Run("Invitation grants access", func(t *testing.T) {
		r := DoGet(t, collab.Client, "/nodes/"+docID)
		assert.Equal(t, "success", r.Status)
	})

	t.Run("Grant to unknown user rejected", func(t *testing.T) {
		r := DoPost(t, owner.Client, "/nodes/"+docID+"/permissions", map[string]any{
			"user_id":    "999999999999999",
			"permission": 1,
		})
		assert.Equal(t, "error", r.Status)
	})

	t.Run("Invalid permission level rejected", func(t *testing.T) {
		r := DoPost(t, owner.Client, "/nodes/"+docID+"/invitations", map[string]any{
			"permission_level": 9,
		})
		assert.Equal(t, "error", r.Status)
		assert.Equal(t, 400, r.StatusCode)
	})
}
