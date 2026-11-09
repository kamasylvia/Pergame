package routes

import (
	"pergame/app"
	"pergame/controllers"
	"pergame/middlewares"
	"pergame/utils"

	"github.com/gin-gonic/gin"
)

func UserSettings(app *app.App, router *gin.RouterGroup) {
	grp := router.Group("/user")
	ctrl := controllers.NewUserSettingsController(app)

	grp.GET("/settings", middlewares.Auth(), utils.WP(ctrl.GetSettings))
	grp.PUT("/settings", middlewares.Auth(), utils.WP(ctrl.SaveSettings))
}
