// ES6 Class annotations via ngInject
(function() {
    class ClassTest1 {
        constructor($log){}
    }
    /** @ngInject */ class ClassTest1_noargs {
        constructor(){}
    }
    /** @ngInject */ class ClassTest1_annotated {
        constructor($log){}
    }
    ClassTest1_annotated.$inject = [
        "$log"
    ];
    class ClassTest1_annotated_constructor {
        /** @ngInject */ constructor($log){}
    }
    ClassTest1_annotated_constructor.$inject = [
        "$log"
    ];
    class ClassTest1_prologue_directive {
        constructor($log){
            "ngInject";
        }
    }
    ClassTest1_prologue_directive.$inject = [
        "$log"
    ];
    let ClassTest2 = class {
        constructor($log){}
    };
    /** @ngInject */ let ClassTest2_noargs = class {
        constructor(){}
    };
    /** @ngInject */ let ClassTest2_annotated = class {
        constructor($log){}
    };
    ClassTest2_annotated.$inject = [
        "$log"
    ];
})();
