// Remove mode should remove existing $inject arrays

Ctrl1.$inject = ["serviceName"];
// @ngInject
function Ctrl1(a) {}

// @ngInject
function Ctrl2(a) {}
Ctrl2.$inject = ["serviceName"];
