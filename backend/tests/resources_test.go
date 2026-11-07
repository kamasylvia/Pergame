package tests

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

// onePixelPNG is a minimal valid 1x1 PNG used for avatar upload tests.
var onePixelPNG = []byte{
	0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d,
	0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
	0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4, 0x89, 0x00, 0x00, 0x00,
	0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x62, 0x00, 0x01, 0x00, 0x00,
	0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49,
	0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
}

// TestResources covers the upload surface: file upload bound to a parent node,
// file update, listing and deletion. Requires the S3-compatible storage to be
// reachable by the server under test (uploads fail otherwise by design).
func TestResources(t *testing.T) {
	user := SignupAndLogin(t, "testpass123")

	// Parent document for uploads.
	r := DoPost(t, user.Client, "/nodes", map[string]any{
		"name": "files-" + user.Username,
		"role": 3,
	})
	assert.Equal(t, "success", r.Status)
	parentID := ResultIDString(t, r)

	t.Run("Unauthenticated upload denied", func(t *testing.T) {
		r := DoPostMultipart(t, NewClient(t), "/resources", "note.txt", []byte("nope"), map[string]string{})
		assert.Equal(t, 401, r.StatusCode)
	})

	var fileID string
	t.Run("Upload text file", func(t *testing.T) {
		r := DoPostMultipart(t, user.Client, "/resources", "note.txt", []byte("hello integration"), map[string]string{
			"parent_id": parentID,
		})
		assert.Equal(t, "success", r.Status)
		fileID = ResultIDString(t, r)
		node := r.Result.(map[string]any)
		assert.Equal(t, "note.txt", node["name"])
	})

	t.Run("Get file node", func(t *testing.T) {
		r := DoGet(t, user.Client, "/nodes/"+fileID)
		assert.Equal(t, "success", r.Status)
		node := resultNodeMap(t, r)
		assert.Equal(t, "note.txt", node["name"])
	})

	t.Run("Update file content", func(t *testing.T) {
		r := DoPostMultipart(t, user.Client, "/resources/"+fileID, "note.txt", []byte("updated content"), map[string]string{})
		assert.Equal(t, "success", r.Status)
	})

	t.Run("List user nodes contains file", func(t *testing.T) {
		r := DoGet(t, user.Client, "/nodes/user/"+user.ID)
		assert.Equal(t, "success", r.Status)
		assert.True(t, ResultArrayContainsID(t, r, fileID))
	})

	t.Run("Upload avatar", func(t *testing.T) {
		r := DoPostMultipart(t, user.Client, "/resources/avatar", "avatar.png", onePixelPNG, map[string]string{})
		assert.Equal(t, "success", r.Status)
	})

	t.Run("Delete file node", func(t *testing.T) {
		r := DoDelete(t, user.Client, "/nodes/"+fileID)
		assert.Equal(t, "success", r.Status)

		after := DoGet(t, user.Client, "/nodes/"+fileID)
		assert.Equal(t, "error", after.Status)
	})
}
