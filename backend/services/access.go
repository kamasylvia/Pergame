package services

import (
	"context"
	"pergame/permissions"
)

func actorFromContext(ctx context.Context) (permissions.Actor, error) {
	actor, ok := permissions.ActorFromContext(ctx)
	if !ok || actor.UserID == 0 {
		return permissions.Actor{}, permissions.ErrUnauthorized
	}
	return actor, nil
}
